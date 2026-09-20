#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(not(feature = "hashbrown"))]
use std::collections::HashMap;
use {
    crate::error::{InvalidSysvarDataError, LiteSVMError},
    log::error,
    parking_lot::{RwLock, RwLockReadGuard},
    solana_account::{AccountSharedData, ReadableAccount, WritableAccount},
    solana_address::Address,
    solana_address_lookup_table_interface::{error::AddressLookupError, state::AddressLookupTable},
    solana_clock::Clock,
    solana_instruction_error::InstructionError,
    solana_loader_v3_interface::state::UpgradeableLoaderState,
    solana_loader_v4_interface::state::LoaderV4State,
    solana_message::{
        v0::{LoadedAddresses, MessageAddressTableLookup},
        AddressLoader,
    },
    solana_nonce as nonce,
    solana_program_runtime::{
        invoke_context::InvokeContext,
        loaded_programs::{
            ProgramCacheForTxBatch, ProgramRuntimeEnvironment, ProgramRuntimeEnvironments,
        },
        program_cache_entry::{
            ProgramCacheEntry, ProgramCacheEntryOwner, DELAY_VISIBILITY_SLOT_OFFSET,
        },
        program_metrics::LoadProgramMetrics,
        solana_sbpf::program::BuiltinProgram,
        sysvar_cache::SysvarCache,
    },
    solana_sdk_ids::{
        bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable, loader_v4, native_loader,
        sysvar::{
            clock::ID as CLOCK_ID, epoch_rewards::ID as EPOCH_REWARDS_ID,
            epoch_schedule::ID as EPOCH_SCHEDULE_ID, last_restart_slot::ID as LAST_RESTART_SLOT_ID,
            rent::ID as RENT_ID, slot_hashes::ID as SLOT_HASHES_ID,
            stake_history::ID as STAKE_HISTORY_ID,
        },
    },
    solana_system_program::{get_system_account_kind, SystemAccountKind},
    solana_sysvar::Sysvar,
    solana_transaction_error::{AddressLoaderError, TransactionError},
    std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    wincode::DeserializeOwned,
};

pub(crate) type AccountsMap = HashMap<Address, AccountSharedData>;

/// Stamps handed to program caches, so a copy of one says which cache and which edit of it
static PROGRAMS_EDITED: AtomicU64 = AtomicU64::new(0);

fn next_programs_stamp() -> u64 {
    PROGRAMS_EDITED.fetch_add(1, Ordering::Relaxed)
}

/// Numbers handed to instances, so a transaction says which one it ran against
static INSTANCES: AtomicU64 = AtomicU64::new(0);

fn next_instance() -> u64 {
    INSTANCES.fetch_add(1, Ordering::Relaxed)
}

const FEES_ID: Address = Address::from_str_const("SysvarFees111111111111111111111111111111111");
const RECENT_BLOCKHASHES_ID: Address =
    Address::from_str_const("SysvarRecentB1ockHashes11111111111111111111");

fn handle_sysvar<T>(
    cache: &mut SysvarCache,
    err_variant: InvalidSysvarDataError,
    account: &AccountSharedData,
    accounts: &AccountsMap,
    address: Address,
) -> Result<(), InvalidSysvarDataError>
where
    T: Sysvar + DeserializeOwned<Dst = T>,
{
    cache.reset();
    cache.fill_missing_entries(|pubkey, set_sysvar| {
        if *pubkey == address {
            set_sysvar(account.data())
        } else if let Some(acc) = accounts.get(pubkey) {
            set_sysvar(acc.data())
        }
    });
    let _parsed = T::deserialize_from(account.data()).map_err(|_| err_variant)?;
    Ok(())
}

/// Account map shared behind a lock; writers lock once and hand the guard to the _locked methods
pub struct AccountsDb {
    accounts: Arc<RwLock<AccountsMap>>,
    programs_cache: ProgramCacheForTxBatch,

    /// What this instance goes by, which no other instance shares
    instance: u64,

    /// What this cache stands at, a fresh number on every instance and every edit
    programs_stamp: u64,
    pub sysvar_cache: SysvarCache,
    pub environments: ProgramRuntimeEnvironments,
    rent: Option<solana_rent::Rent>,
}

impl Clone for AccountsDb {
    fn clone(&self) -> Self {
        Self {
            accounts: Arc::new(RwLock::new(self.accounts.read().clone())),
            programs_cache: self.programs_cache.clone(),
            instance: next_instance(),
            programs_stamp: next_programs_stamp(),
            sysvar_cache: self.sysvar_cache.clone(),
            environments: ProgramRuntimeEnvironments::new(
                self.environments.get_env_for_execution().clone(),
                self.environments.get_env_for_deployment().clone(),
            ),
            rent: self.rent.clone(),
        }
    }
}

impl Default for AccountsDb {
    fn default() -> Self {
        let env = ProgramRuntimeEnvironment::from(
            BuiltinProgram::<InvokeContext<'static, 'static>>::new_mock(),
        );

        Self {
            accounts: Arc::new(RwLock::new(AccountsMap::default())),
            programs_cache: ProgramCacheForTxBatch::new(DELAY_VISIBILITY_SLOT_OFFSET),
            instance: next_instance(),
            programs_stamp: next_programs_stamp(),
            sysvar_cache: SysvarCache::default(),
            environments: ProgramRuntimeEnvironments::new(env.clone(), env),
            rent: None,
        }
    }
}

impl AccountsDb {
    /// The programs a transaction runs against
    pub(crate) fn programs(&self) -> &ProgramCacheForTxBatch {
        &self.programs_cache
    }

    /// The programs for an edit, which every copy taken of them has to be taken again after
    pub(crate) fn programs_mut(&mut self) -> &mut ProgramCacheForTxBatch {
        self.programs_stamp = next_programs_stamp();
        &mut self.programs_cache
    }

    /// Which cache and which edit of it a copy would be taken from
    pub(crate) fn programs_stamp(&self) -> u64 {
        self.programs_stamp
    }

    /// The slot the program cache stands at, which every copy is set to after it is taken
    pub(crate) fn programs_slot(&self) -> u64 {
        self.programs_cache.slot()
    }

    /// Move the cache to `slot`, which hands out no stamp because it retakes no copy
    ///
    /// The cache stands one slot ahead of the chain, so a program deployed in `slot` is usable
    /// in it.
    pub(crate) fn set_programs_slot(&mut self, slot: u64) {
        self.programs_cache
            .set_slot_for_tests(slot.saturating_add(DELAY_VISIBILITY_SLOT_OFFSET));
    }

    /// The instance a transaction runs against, which is the one that may commit it
    pub(crate) fn instance(&self) -> u64 {
        self.instance
    }

    pub fn get_account(&self, pubkey: &Address) -> Option<AccountSharedData> {
        self.accounts.read().get(pubkey).cloned()
    }

    pub fn contains_account(&self, pubkey: &Address) -> bool {
        self.accounts.read().contains_key(pubkey)
    }

    /// Runs f over every account under one read guard
    pub fn scan_accounts<R>(
        &self,
        f: impl FnOnce(&mut dyn Iterator<Item = (&Address, &AccountSharedData)>) -> R,
    ) -> R {
        let map = self.accounts.read();
        let mut iter = map.iter();
        f(&mut iter)
    }

    /// The account map under one guard, for a caller loading a whole transaction at once
    pub(crate) fn read_accounts(&self) -> RwLockReadGuard<'_, AccountsMap> {
        self.accounts.read()
    }

    pub(crate) fn share_accounts(&self) -> Arc<RwLock<AccountsMap>> {
        Arc::clone(&self.accounts)
    }

    /// Clone out keys and their owners under one read guard
    pub(crate) fn copy_working_set<'a>(
        &self,
        keys: impl Iterator<Item = &'a Address>,
    ) -> AccountsMap {
        let map = self.accounts.read();
        let mut working = AccountsMap::default();
        for key in keys {
            let Some(account) = map.get(key) else {
                continue;
            };
            let owner = *account.owner();
            working.insert(*key, account.clone());
            if let Some(owner_account) = map.get(&owner) {
                working.insert(owner, owner_account.clone());
            }
        }
        working
    }

    /// A db over working alone, the map an off-lock execution reads
    pub(crate) fn with_working_set(&self, working: AccountsMap) -> Self {
        Self {
            accounts: Arc::new(RwLock::new(working)),
            programs_cache: self.programs_cache.clone(),
            instance: next_instance(),
            programs_stamp: next_programs_stamp(),
            sysvar_cache: self.sysvar_cache.clone(),
            environments: ProgramRuntimeEnvironments::new(
                self.environments.get_env_for_execution().clone(),
                self.environments.get_env_for_deployment().clone(),
            ),
            rent: self.rent.clone(),
        }
    }

    pub(crate) fn cached_rent(&self) -> solana_rent::Rent {
        self.rent
            .clone()
            .unwrap_or_else(|| self.sysvar_cache.get_rent().unwrap().as_ref().clone())
    }

    /// We should only use this when we know we're not touching any executable or sysvar accounts,
    /// or have already handled such cases.
    pub(crate) fn add_account_no_checks(&mut self, pubkey: Address, account: AccountSharedData) {
        self.accounts.write().insert(pubkey, account);
    }

    pub(crate) fn add_account(
        &mut self,
        pubkey: Address,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        let accounts = Arc::clone(&self.accounts);
        let mut map = accounts.write();
        self.add_account_locked(&mut map, pubkey, account)
    }

    fn add_account_locked(
        &mut self,
        map: &mut AccountsMap,
        pubkey: Address,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        if account.executable()
            && pubkey != Address::default()
            && account.owner() != &native_loader::ID
        {
            let loaded_program = self.load_program(map, &account)?;
            self.programs_mut()
                .replenish(pubkey, Arc::new(loaded_program));
        } else {
            self.maybe_handle_sysvar_account(map, pubkey, &account)?;
        }
        if account.lamports() == 0 {
            map.remove(&pubkey);
        } else {
            map.insert(pubkey, account);
        }
        Ok(())
    }

    fn maybe_handle_sysvar_account(
        &mut self,
        map: &AccountsMap,
        pubkey: Address,
        account: &AccountSharedData,
    ) -> Result<(), InvalidSysvarDataError> {
        use InvalidSysvarDataError::{
            EpochRewards, EpochSchedule, Fees, RecentBlockhashes, SlotHashes, StakeHistory,
        };
        if account.owner() != &solana_sdk_ids::sysvar::id() {
            return Ok(());
        }
        // Fixed-width sysvars update in place; variable-width ones still reset and refill
        #[allow(deprecated)]
        match pubkey {
            CLOCK_ID => {
                let parsed = Clock::deserialize_from(account.data())
                    .map_err(|_| InvalidSysvarDataError::Clock)?;
                self.set_programs_slot(parsed.slot);
                self.sysvar_cache.set_sysvar_for_tests(&parsed);
            }
            EPOCH_REWARDS_ID => {
                let parsed = solana_epoch_rewards::EpochRewards::deserialize_from(account.data())
                    .map_err(|_| EpochRewards)?;
                self.sysvar_cache.set_sysvar_for_tests(&parsed);
            }
            EPOCH_SCHEDULE_ID => {
                let parsed = solana_epoch_schedule::EpochSchedule::deserialize_from(account.data())
                    .map_err(|_| EpochSchedule)?;
                self.sysvar_cache.set_sysvar_for_tests(&parsed);
            }
            FEES_ID => {
                handle_sysvar::<solana_sysvar::fees::Fees>(
                    &mut self.sysvar_cache,
                    Fees,
                    account,
                    map,
                    pubkey,
                )?;
            }
            LAST_RESTART_SLOT_ID => {
                let parsed = solana_sysvar::last_restart_slot::LastRestartSlot::deserialize_from(
                    account.data(),
                )
                .map_err(|_| InvalidSysvarDataError::LastRestartSlot)?;
                self.sysvar_cache.set_sysvar_for_tests(&parsed);
            }
            RECENT_BLOCKHASHES_ID => {
                handle_sysvar::<solana_sysvar::recent_blockhashes::RecentBlockhashes>(
                    &mut self.sysvar_cache,
                    RecentBlockhashes,
                    account,
                    map,
                    pubkey,
                )?;
            }
            RENT_ID => {
                let parsed = solana_rent::Rent::deserialize_from(account.data())
                    .map_err(|_| InvalidSysvarDataError::Rent)?;
                self.sysvar_cache.set_sysvar_for_tests(&parsed);
                self.rent = Some(parsed);
            }
            SLOT_HASHES_ID => {
                handle_sysvar::<solana_slot_hashes::SlotHashes>(
                    &mut self.sysvar_cache,
                    SlotHashes,
                    account,
                    map,
                    pubkey,
                )?;
            }
            STAKE_HISTORY_ID => {
                handle_sysvar::<solana_stake_history::StakeHistory>(
                    &mut self.sysvar_cache,
                    StakeHistory,
                    account,
                    map,
                    pubkey,
                )?;
            }
            _ => {}
        };
        Ok(())
    }

    /// Rebuilds the sysvar cache from account data already in the map
    #[cfg(feature = "persistence-internal")]
    pub(crate) fn rebuild_sysvar_cache(&mut self) {
        let accounts = Arc::clone(&self.accounts);
        let map = accounts.read();
        self.sysvar_cache.reset();
        self.sysvar_cache
            .fill_missing_entries(|pubkey, set_sysvar| {
                if let Some(acc) = map.get(pubkey) {
                    set_sysvar(acc.data())
                }
            });
        if let Ok(clock) = self.sysvar_cache.get_clock() {
            self.set_programs_slot(clock.slot);
        }
        self.rent = self
            .sysvar_cache
            .get_rent()
            .ok()
            .map(|rent| rent.as_ref().clone());
    }

    /// Scans all accounts for executable BPF programs and loads them into the program cache.
    #[cfg(feature = "persistence-internal")]
    pub(crate) fn load_all_existing_programs(&mut self) -> Result<(), LiteSVMError> {
        let accounts = Arc::clone(&self.accounts);
        let map = accounts.read();
        let executable_accounts: Vec<(Address, AccountSharedData)> = map
            .iter()
            .filter(|(_, acc)| acc.executable() && acc.owner() != &native_loader::ID)
            .map(|(k, acc)| (*k, acc.clone()))
            .collect();

        for (key, account) in executable_accounts {
            let loaded = self.load_program(&map, &account)?;
            self.programs_mut().replenish(key, Arc::new(loaded));
        }
        Ok(())
    }

    /// Applies a transaction's post accounts under one write guard
    pub(crate) fn sync_accounts(
        &mut self,
        mut accounts: Vec<(Address, AccountSharedData)>,
    ) -> Result<(), LiteSVMError> {
        // need to add programdata accounts first if there are any
        itertools::partition(&mut accounts, |x| {
            x.1.owner() == &bpf_loader_upgradeable::id()
                && x.1.data().first().is_some_and(|byte| *byte == 3)
        });
        let shared = Arc::clone(&self.accounts);
        let mut map = shared.write();
        for (address, acc) in accounts {
            self.add_account_locked(&mut map, address, acc)?;
        }
        Ok(())
    }

    fn load_program(
        &self,
        map: &AccountsMap,
        program_account: &AccountSharedData,
    ) -> Result<ProgramCacheEntry, InstructionError> {
        let metrics = &mut LoadProgramMetrics::default();

        let owner = program_account.owner();
        let program_runtime_for_execution = self.environments.get_env_for_execution().clone();
        let slot = self.sysvar_cache.get_clock().map(|c| c.slot).unwrap_or(0);

        if bpf_loader::check_id(owner) || bpf_loader_deprecated::check_id(owner) {
            ProgramCacheEntry::load(
                owner,
                program_runtime_for_execution,
                slot,
                program_account.data(),
                metrics,
            )
            .map_err(|e| {
                error!("Failed to load program: {e:?}");
                InstructionError::InvalidAccountData
            })
        } else if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) = UpgradeableLoaderState::deserialize_from(program_account.data())
            else {
                error!(
                    "Program account data does not deserialize to UpgradeableLoaderState::Program"
                );
                return Err(InstructionError::InvalidAccountData);
            };
            let Some(programdata_account) = map.get(&programdata_address) else {
                return Ok(ProgramCacheEntry::new_closed_tombstone(
                    slot,
                    ProgramCacheEntryOwner::LoaderV3,
                ));
            };
            let program_data = programdata_account.data();
            if let Some(programdata) =
                program_data.get(UpgradeableLoaderState::size_of_programdata_metadata()..)
            {
                ProgramCacheEntry::load(
                    owner,
                    program_runtime_for_execution,
                    slot,
                    programdata,
                    metrics).map_err(|e| {
                        error!("Error encountered when calling ProgramCacheEntry::load() for bpf_loader_upgradeable: {e:?}");
                        InstructionError::InvalidAccountData
                    })
            } else {
                error!("Index out of bounds using bpf_loader_upgradeable.");
                Err(InstructionError::InvalidAccountData)
            }
        } else if loader_v4::check_id(owner) {
            if let Some(elf_bytes) = program_account
                .data()
                .get(LoaderV4State::program_data_offset()..)
            {
                ProgramCacheEntry::load(
                    &loader_v4::id(),
                    program_runtime_for_execution,
                    slot,
                    elf_bytes,
                    metrics,
                )
                .map_err(|_| {
                    error!(
                        "Error encountered when calling ProgramCacheEntry::load() for loader_v4."
                    );
                    InstructionError::InvalidAccountData
                })
            } else {
                error!("Index out of bounds using loader_v4.");
                Err(InstructionError::InvalidAccountData)
            }
        } else {
            error!("Owner does not match any expected loader.");
            Err(InstructionError::IncorrectProgramId)
        }
    }

    fn load_lookup_table_addresses(
        &self,
        address_table_lookup: &MessageAddressTableLookup,
    ) -> std::result::Result<LoadedAddresses, AddressLookupError> {
        let map = self.accounts.read();
        let table_account = map
            .get(&address_table_lookup.account_key)
            .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

        if table_account.owner() == &solana_sdk_ids::address_lookup_table::id() {
            let slot_hashes = self.sysvar_cache.get_slot_hashes().unwrap();
            let current_slot = self.sysvar_cache.get_clock().unwrap().slot;
            let lookup_table = AddressLookupTable::deserialize(table_account.data())
                .map_err(|_ix_err| AddressLookupError::InvalidAccountData)?;

            Ok(LoadedAddresses {
                writable: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.writable_indexes,
                    &slot_hashes,
                )?,
                readonly: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.readonly_indexes,
                    &slot_hashes,
                )?,
            })
        } else {
            Err(AddressLookupError::InvalidAccountOwner)
        }
    }

    pub(crate) fn withdraw(
        &mut self,
        address: &Address,
        lamports: u64,
    ) -> solana_transaction_error::TransactionResult<()> {
        let accounts = Arc::clone(&self.accounts);
        let mut map = accounts.write();
        match map.get_mut(address) {
            Some(account) => {
                let min_balance = match get_system_account_kind(account) {
                    Some(SystemAccountKind::Nonce) => self
                        .sysvar_cache
                        .get_rent()
                        .unwrap()
                        .minimum_balance(nonce::state::State::size()),
                    _ => 0,
                };

                lamports
                    .checked_add(min_balance)
                    .filter(|required_balance| *required_balance <= account.lamports())
                    .ok_or(TransactionError::InsufficientFundsForFee)?;
                account
                    .checked_sub_lamports(lamports)
                    .map_err(|_| TransactionError::InsufficientFundsForFee)?;

                Ok(())
            }
            None => {
                error!("Account {address} not found when trying to withdraw fee.");
                Err(TransactionError::AccountNotFound)
            }
        }
    }

    /// Returns a copy of the ELF bytes for this account
    /// Fails if the account is not a program account.
    pub fn try_program_elf_bytes(
        &self,
        program_key: &Address,
    ) -> std::result::Result<Vec<u8>, InstructionError> {
        let map = self.accounts.read();
        let program_account = map
            .get(program_key)
            .ok_or(InstructionError::MissingAccount)?;
        let owner = program_account.owner();

        if bpf_loader::check_id(owner) || bpf_loader_deprecated::check_id(owner) {
            Ok(program_account.data().to_vec())
        } else if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) = UpgradeableLoaderState::deserialize_from(program_account.data())
            else {
                return Err(InstructionError::InvalidAccountData);
            };
            let programdata_account = map.get(&programdata_address).ok_or_else(|| {
                error!("Program data account {programdata_address} not found");
                InstructionError::MissingAccount
            })?;
            let program_data = programdata_account.data();
            if let Some(programdata) =
                program_data.get(UpgradeableLoaderState::size_of_programdata_metadata()..)
            {
                Ok(programdata.to_vec())
            } else {
                error!("Index out of bounds using bpf_loader_upgradeable.");
                Err(InstructionError::InvalidAccountData)
            }
        } else if loader_v4::check_id(owner) {
            if let Some(elf_bytes) = program_account
                .data()
                .get(LoaderV4State::program_data_offset()..)
            {
                Ok(elf_bytes.to_vec())
            } else {
                error!("Index out of bounds using loader_v4.");
                Err(InstructionError::InvalidAccountData)
            }
        } else {
            error!("Owner does not match any expected loader.");
            Err(InstructionError::IncorrectProgramId)
        }
    }
}

fn into_address_loader_error(err: AddressLookupError) -> AddressLoaderError {
    match err {
        AddressLookupError::LookupTableAccountNotFound => {
            AddressLoaderError::LookupTableAccountNotFound
        }
        AddressLookupError::InvalidAccountOwner => AddressLoaderError::InvalidAccountOwner,
        AddressLookupError::InvalidAccountData => AddressLoaderError::InvalidAccountData,
        AddressLookupError::InvalidLookupIndex => AddressLoaderError::InvalidLookupIndex,
    }
}

impl AddressLoader for &AccountsDb {
    fn load_addresses(
        self,
        lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        lookups
            .iter()
            .map(|lookup| {
                self.load_lookup_table_addresses(lookup)
                    .map_err(into_address_loader_error)
            })
            .collect()
    }
}
