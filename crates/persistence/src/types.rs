use {
    agave_feature_set::FeatureSet,
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_fee_structure::{FeeBin, FeeStructure},
    solana_hash::Hash,
    solana_message::{
        compiled_instruction::CompiledInstruction, inner_instruction::InnerInstruction,
    },
    solana_signature::Signature,
    solana_transaction_context::transaction::TransactionReturnData,
    solana_transaction_error::TransactionError,
    wincode::{SchemaRead, SchemaWrite},
};

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeeBinWire {
    pub limit: u64,
    pub fee: u64,
}

impl From<FeeBin> for FeeBinWire {
    fn from(value: FeeBin) -> Self {
        Self {
            limit: value.limit,
            fee: value.fee,
        }
    }
}

impl From<FeeBinWire> for FeeBin {
    fn from(value: FeeBinWire) -> Self {
        Self {
            limit: value.limit,
            fee: value.fee,
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeeStructureWire {
    pub lamports_per_signature: u64,
    pub lamports_per_write_lock: u64,
    pub compute_fee_bins: Vec<FeeBinWire>,
}

impl From<FeeStructure> for FeeStructureWire {
    fn from(value: FeeStructure) -> Self {
        Self {
            lamports_per_signature: value.lamports_per_signature,
            lamports_per_write_lock: value.lamports_per_write_lock,
            compute_fee_bins: value.compute_fee_bins.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<FeeStructureWire> for FeeStructure {
    fn from(value: FeeStructureWire) -> Self {
        Self {
            lamports_per_signature: value.lamports_per_signature,
            lamports_per_write_lock: value.lamports_per_write_lock,
            compute_fee_bins: value.compute_fee_bins.into_iter().map(Into::into).collect(),
        }
    }
}

/// Compute-budget layout written by persistence version 1 (LiteSVM v0.15.2).
/// Solana 4.2 removed the two modular-exponentiation fields in the middle of
/// the layout, so version 1 must be decoded into this exact historical shape
/// before it can be migrated to the current [`ComputeBudget`].
#[derive(SchemaRead)]
pub(crate) struct ComputeBudgetV1 {
    pub compute_unit_limit: u64,
    pub log_64_units: u64,
    pub create_program_address_units: u64,
    pub invoke_units: u64,
    pub max_instruction_stack_depth: usize,
    pub max_instruction_trace_length: usize,
    pub sha256_base_cost: u64,
    pub sha256_byte_cost: u64,
    pub sha256_max_slices: u64,
    pub max_call_depth: usize,
    pub stack_frame_size: usize,
    pub log_pubkey_units: u64,
    pub cpi_bytes_per_unit: u64,
    pub sysvar_base_cost: u64,
    pub secp256k1_recover_cost: u64,
    pub syscall_base_cost: u64,
    pub curve25519_edwards_validate_point_cost: u64,
    pub curve25519_edwards_add_cost: u64,
    pub curve25519_edwards_subtract_cost: u64,
    pub curve25519_edwards_multiply_cost: u64,
    pub curve25519_edwards_msm_base_cost: u64,
    pub curve25519_edwards_msm_incremental_cost: u64,
    pub curve25519_ristretto_validate_point_cost: u64,
    pub curve25519_ristretto_add_cost: u64,
    pub curve25519_ristretto_subtract_cost: u64,
    pub curve25519_ristretto_multiply_cost: u64,
    pub curve25519_ristretto_msm_base_cost: u64,
    pub curve25519_ristretto_msm_incremental_cost: u64,
    pub heap_size: u32,
    pub heap_cost: u64,
    pub mem_op_base_cost: u64,
    pub alt_bn128_g1_addition_cost: u64,
    pub alt_bn128_g2_addition_cost: u64,
    pub alt_bn128_g1_multiplication_cost: u64,
    pub alt_bn128_g2_multiplication_cost: u64,
    pub alt_bn128_pairing_one_pair_cost_first: u64,
    pub alt_bn128_pairing_one_pair_cost_other: u64,
    pub big_modular_exponentiation_base_cost: u64,
    pub big_modular_exponentiation_cost_divisor: u64,
    pub poseidon_cost_coefficient_a: u64,
    pub poseidon_cost_coefficient_c: u64,
    pub get_remaining_compute_units_cost: u64,
    pub alt_bn128_g1_compress: u64,
    pub alt_bn128_g1_decompress: u64,
    pub alt_bn128_g2_compress: u64,
    pub alt_bn128_g2_decompress: u64,
    pub bls12_381_g1_add_cost: u64,
    pub bls12_381_g2_add_cost: u64,
    pub bls12_381_g1_subtract_cost: u64,
    pub bls12_381_g2_subtract_cost: u64,
    pub bls12_381_g1_multiply_cost: u64,
    pub bls12_381_g2_multiply_cost: u64,
    pub bls12_381_g1_decompress_cost: u64,
    pub bls12_381_g2_decompress_cost: u64,
    pub bls12_381_g1_validate_cost: u64,
    pub bls12_381_g2_validate_cost: u64,
    pub bls12_381_one_pair_cost: u64,
    pub bls12_381_additional_pair_cost: u64,
}

impl From<ComputeBudgetV1> for ComputeBudget {
    fn from(value: ComputeBudgetV1) -> Self {
        Self {
            compute_unit_limit: value.compute_unit_limit,
            log_64_units: value.log_64_units,
            create_program_address_units: value.create_program_address_units,
            invoke_units: value.invoke_units,
            max_instruction_stack_depth: value.max_instruction_stack_depth,
            max_instruction_trace_length: value.max_instruction_trace_length,
            sha256_base_cost: value.sha256_base_cost,
            sha256_byte_cost: value.sha256_byte_cost,
            sha256_max_slices: value.sha256_max_slices,
            max_call_depth: value.max_call_depth,
            stack_frame_size: value.stack_frame_size,
            log_pubkey_units: value.log_pubkey_units,
            cpi_bytes_per_unit: value.cpi_bytes_per_unit,
            sysvar_base_cost: value.sysvar_base_cost,
            secp256k1_recover_cost: value.secp256k1_recover_cost,
            syscall_base_cost: value.syscall_base_cost,
            curve25519_edwards_validate_point_cost: value.curve25519_edwards_validate_point_cost,
            curve25519_edwards_add_cost: value.curve25519_edwards_add_cost,
            curve25519_edwards_subtract_cost: value.curve25519_edwards_subtract_cost,
            curve25519_edwards_multiply_cost: value.curve25519_edwards_multiply_cost,
            curve25519_edwards_msm_base_cost: value.curve25519_edwards_msm_base_cost,
            curve25519_edwards_msm_incremental_cost: value.curve25519_edwards_msm_incremental_cost,
            curve25519_ristretto_validate_point_cost: value
                .curve25519_ristretto_validate_point_cost,
            curve25519_ristretto_add_cost: value.curve25519_ristretto_add_cost,
            curve25519_ristretto_subtract_cost: value.curve25519_ristretto_subtract_cost,
            curve25519_ristretto_multiply_cost: value.curve25519_ristretto_multiply_cost,
            curve25519_ristretto_msm_base_cost: value.curve25519_ristretto_msm_base_cost,
            curve25519_ristretto_msm_incremental_cost: value
                .curve25519_ristretto_msm_incremental_cost,
            heap_size: value.heap_size,
            heap_cost: value.heap_cost,
            mem_op_base_cost: value.mem_op_base_cost,
            alt_bn128_g1_addition_cost: value.alt_bn128_g1_addition_cost,
            alt_bn128_g2_addition_cost: value.alt_bn128_g2_addition_cost,
            alt_bn128_g1_multiplication_cost: value.alt_bn128_g1_multiplication_cost,
            alt_bn128_g2_multiplication_cost: value.alt_bn128_g2_multiplication_cost,
            alt_bn128_pairing_one_pair_cost_first: value.alt_bn128_pairing_one_pair_cost_first,
            alt_bn128_pairing_one_pair_cost_other: value.alt_bn128_pairing_one_pair_cost_other,
            poseidon_cost_coefficient_a: value.poseidon_cost_coefficient_a,
            poseidon_cost_coefficient_c: value.poseidon_cost_coefficient_c,
            get_remaining_compute_units_cost: value.get_remaining_compute_units_cost,
            alt_bn128_g1_compress: value.alt_bn128_g1_compress,
            alt_bn128_g1_decompress: value.alt_bn128_g1_decompress,
            alt_bn128_g2_compress: value.alt_bn128_g2_compress,
            alt_bn128_g2_decompress: value.alt_bn128_g2_decompress,
            bls12_381_g1_add_cost: value.bls12_381_g1_add_cost,
            bls12_381_g2_add_cost: value.bls12_381_g2_add_cost,
            bls12_381_g1_subtract_cost: value.bls12_381_g1_subtract_cost,
            bls12_381_g2_subtract_cost: value.bls12_381_g2_subtract_cost,
            bls12_381_g1_multiply_cost: value.bls12_381_g1_multiply_cost,
            bls12_381_g2_multiply_cost: value.bls12_381_g2_multiply_cost,
            bls12_381_g1_decompress_cost: value.bls12_381_g1_decompress_cost,
            bls12_381_g2_decompress_cost: value.bls12_381_g2_decompress_cost,
            bls12_381_g1_validate_cost: value.bls12_381_g1_validate_cost,
            bls12_381_g2_validate_cost: value.bls12_381_g2_validate_cost,
            bls12_381_one_pair_cost: value.bls12_381_one_pair_cost,
            bls12_381_additional_pair_cost: value.bls12_381_additional_pair_cost,
            big_modular_exponentiation_base_cost: value.big_modular_exponentiation_base_cost,
            big_modular_exponentiation_cost_divisor: value.big_modular_exponentiation_cost_divisor,
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct ComputeBudgetWire {
    pub compute_unit_limit: u64,
    pub log_64_units: u64,
    pub create_program_address_units: u64,
    pub invoke_units: u64,
    pub max_instruction_stack_depth: usize,
    pub max_instruction_trace_length: usize,
    pub sha256_base_cost: u64,
    pub sha256_byte_cost: u64,
    pub sha256_max_slices: u64,
    pub max_call_depth: usize,
    pub stack_frame_size: usize,
    pub log_pubkey_units: u64,
    pub cpi_bytes_per_unit: u64,
    pub sysvar_base_cost: u64,
    pub secp256k1_recover_cost: u64,
    pub syscall_base_cost: u64,
    pub curve25519_edwards_validate_point_cost: u64,
    pub curve25519_edwards_add_cost: u64,
    pub curve25519_edwards_subtract_cost: u64,
    pub curve25519_edwards_multiply_cost: u64,
    pub curve25519_edwards_msm_base_cost: u64,
    pub curve25519_edwards_msm_incremental_cost: u64,
    pub curve25519_ristretto_validate_point_cost: u64,
    pub curve25519_ristretto_add_cost: u64,
    pub curve25519_ristretto_subtract_cost: u64,
    pub curve25519_ristretto_multiply_cost: u64,
    pub curve25519_ristretto_msm_base_cost: u64,
    pub curve25519_ristretto_msm_incremental_cost: u64,
    pub heap_size: u32,
    pub heap_cost: u64,
    pub mem_op_base_cost: u64,
    pub alt_bn128_g1_addition_cost: u64,
    pub alt_bn128_g2_addition_cost: u64,
    pub alt_bn128_g1_multiplication_cost: u64,
    pub alt_bn128_g2_multiplication_cost: u64,
    pub alt_bn128_pairing_one_pair_cost_first: u64,
    pub alt_bn128_pairing_one_pair_cost_other: u64,
    pub poseidon_cost_coefficient_a: u64,
    pub poseidon_cost_coefficient_c: u64,
    pub get_remaining_compute_units_cost: u64,
    pub alt_bn128_g1_compress: u64,
    pub alt_bn128_g1_decompress: u64,
    pub alt_bn128_g2_compress: u64,
    pub alt_bn128_g2_decompress: u64,
    pub bls12_381_g1_add_cost: u64,
    pub bls12_381_g2_add_cost: u64,
    pub bls12_381_g1_subtract_cost: u64,
    pub bls12_381_g2_subtract_cost: u64,
    pub bls12_381_g1_multiply_cost: u64,
    pub bls12_381_g2_multiply_cost: u64,
    pub bls12_381_g1_decompress_cost: u64,
    pub bls12_381_g2_decompress_cost: u64,
    pub bls12_381_g1_validate_cost: u64,
    pub bls12_381_g2_validate_cost: u64,
    pub bls12_381_one_pair_cost: u64,
    pub bls12_381_additional_pair_cost: u64,
}

impl From<ComputeBudget> for ComputeBudgetWire {
    fn from(value: ComputeBudget) -> Self {
        Self {
            compute_unit_limit: value.compute_unit_limit,
            log_64_units: value.log_64_units,
            create_program_address_units: value.create_program_address_units,
            invoke_units: value.invoke_units,
            max_instruction_stack_depth: value.max_instruction_stack_depth,
            max_instruction_trace_length: value.max_instruction_trace_length,
            sha256_base_cost: value.sha256_base_cost,
            sha256_byte_cost: value.sha256_byte_cost,
            sha256_max_slices: value.sha256_max_slices,
            max_call_depth: value.max_call_depth,
            stack_frame_size: value.stack_frame_size,
            log_pubkey_units: value.log_pubkey_units,
            cpi_bytes_per_unit: value.cpi_bytes_per_unit,
            sysvar_base_cost: value.sysvar_base_cost,
            secp256k1_recover_cost: value.secp256k1_recover_cost,
            syscall_base_cost: value.syscall_base_cost,
            curve25519_edwards_validate_point_cost: value.curve25519_edwards_validate_point_cost,
            curve25519_edwards_add_cost: value.curve25519_edwards_add_cost,
            curve25519_edwards_subtract_cost: value.curve25519_edwards_subtract_cost,
            curve25519_edwards_multiply_cost: value.curve25519_edwards_multiply_cost,
            curve25519_edwards_msm_base_cost: value.curve25519_edwards_msm_base_cost,
            curve25519_edwards_msm_incremental_cost: value.curve25519_edwards_msm_incremental_cost,
            curve25519_ristretto_validate_point_cost: value
                .curve25519_ristretto_validate_point_cost,
            curve25519_ristretto_add_cost: value.curve25519_ristretto_add_cost,
            curve25519_ristretto_subtract_cost: value.curve25519_ristretto_subtract_cost,
            curve25519_ristretto_multiply_cost: value.curve25519_ristretto_multiply_cost,
            curve25519_ristretto_msm_base_cost: value.curve25519_ristretto_msm_base_cost,
            curve25519_ristretto_msm_incremental_cost: value
                .curve25519_ristretto_msm_incremental_cost,
            heap_size: value.heap_size,
            heap_cost: value.heap_cost,
            mem_op_base_cost: value.mem_op_base_cost,
            alt_bn128_g1_addition_cost: value.alt_bn128_g1_addition_cost,
            alt_bn128_g2_addition_cost: value.alt_bn128_g2_addition_cost,
            alt_bn128_g1_multiplication_cost: value.alt_bn128_g1_multiplication_cost,
            alt_bn128_g2_multiplication_cost: value.alt_bn128_g2_multiplication_cost,
            alt_bn128_pairing_one_pair_cost_first: value.alt_bn128_pairing_one_pair_cost_first,
            alt_bn128_pairing_one_pair_cost_other: value.alt_bn128_pairing_one_pair_cost_other,
            poseidon_cost_coefficient_a: value.poseidon_cost_coefficient_a,
            poseidon_cost_coefficient_c: value.poseidon_cost_coefficient_c,
            get_remaining_compute_units_cost: value.get_remaining_compute_units_cost,
            alt_bn128_g1_compress: value.alt_bn128_g1_compress,
            alt_bn128_g1_decompress: value.alt_bn128_g1_decompress,
            alt_bn128_g2_compress: value.alt_bn128_g2_compress,
            alt_bn128_g2_decompress: value.alt_bn128_g2_decompress,
            bls12_381_g1_add_cost: value.bls12_381_g1_add_cost,
            bls12_381_g2_add_cost: value.bls12_381_g2_add_cost,
            bls12_381_g1_subtract_cost: value.bls12_381_g1_subtract_cost,
            bls12_381_g2_subtract_cost: value.bls12_381_g2_subtract_cost,
            bls12_381_g1_multiply_cost: value.bls12_381_g1_multiply_cost,
            bls12_381_g2_multiply_cost: value.bls12_381_g2_multiply_cost,
            bls12_381_g1_decompress_cost: value.bls12_381_g1_decompress_cost,
            bls12_381_g2_decompress_cost: value.bls12_381_g2_decompress_cost,
            bls12_381_g1_validate_cost: value.bls12_381_g1_validate_cost,
            bls12_381_g2_validate_cost: value.bls12_381_g2_validate_cost,
            bls12_381_one_pair_cost: value.bls12_381_one_pair_cost,
            bls12_381_additional_pair_cost: value.bls12_381_additional_pair_cost,
        }
    }
}

impl From<ComputeBudgetWire> for ComputeBudget {
    fn from(value: ComputeBudgetWire) -> Self {
        Self {
            compute_unit_limit: value.compute_unit_limit,
            log_64_units: value.log_64_units,
            create_program_address_units: value.create_program_address_units,
            invoke_units: value.invoke_units,
            max_instruction_stack_depth: value.max_instruction_stack_depth,
            max_instruction_trace_length: value.max_instruction_trace_length,
            sha256_base_cost: value.sha256_base_cost,
            sha256_byte_cost: value.sha256_byte_cost,
            sha256_max_slices: value.sha256_max_slices,
            max_call_depth: value.max_call_depth,
            stack_frame_size: value.stack_frame_size,
            log_pubkey_units: value.log_pubkey_units,
            cpi_bytes_per_unit: value.cpi_bytes_per_unit,
            sysvar_base_cost: value.sysvar_base_cost,
            secp256k1_recover_cost: value.secp256k1_recover_cost,
            syscall_base_cost: value.syscall_base_cost,
            curve25519_edwards_validate_point_cost: value.curve25519_edwards_validate_point_cost,
            curve25519_edwards_add_cost: value.curve25519_edwards_add_cost,
            curve25519_edwards_subtract_cost: value.curve25519_edwards_subtract_cost,
            curve25519_edwards_multiply_cost: value.curve25519_edwards_multiply_cost,
            curve25519_edwards_msm_base_cost: value.curve25519_edwards_msm_base_cost,
            curve25519_edwards_msm_incremental_cost: value.curve25519_edwards_msm_incremental_cost,
            curve25519_ristretto_validate_point_cost: value
                .curve25519_ristretto_validate_point_cost,
            curve25519_ristretto_add_cost: value.curve25519_ristretto_add_cost,
            curve25519_ristretto_subtract_cost: value.curve25519_ristretto_subtract_cost,
            curve25519_ristretto_multiply_cost: value.curve25519_ristretto_multiply_cost,
            curve25519_ristretto_msm_base_cost: value.curve25519_ristretto_msm_base_cost,
            curve25519_ristretto_msm_incremental_cost: value
                .curve25519_ristretto_msm_incremental_cost,
            heap_size: value.heap_size,
            heap_cost: value.heap_cost,
            mem_op_base_cost: value.mem_op_base_cost,
            alt_bn128_g1_addition_cost: value.alt_bn128_g1_addition_cost,
            alt_bn128_g2_addition_cost: value.alt_bn128_g2_addition_cost,
            alt_bn128_g1_multiplication_cost: value.alt_bn128_g1_multiplication_cost,
            alt_bn128_g2_multiplication_cost: value.alt_bn128_g2_multiplication_cost,
            alt_bn128_pairing_one_pair_cost_first: value.alt_bn128_pairing_one_pair_cost_first,
            alt_bn128_pairing_one_pair_cost_other: value.alt_bn128_pairing_one_pair_cost_other,
            poseidon_cost_coefficient_a: value.poseidon_cost_coefficient_a,
            poseidon_cost_coefficient_c: value.poseidon_cost_coefficient_c,
            get_remaining_compute_units_cost: value.get_remaining_compute_units_cost,
            alt_bn128_g1_compress: value.alt_bn128_g1_compress,
            alt_bn128_g1_decompress: value.alt_bn128_g1_decompress,
            alt_bn128_g2_compress: value.alt_bn128_g2_compress,
            alt_bn128_g2_decompress: value.alt_bn128_g2_decompress,
            bls12_381_g1_add_cost: value.bls12_381_g1_add_cost,
            bls12_381_g2_add_cost: value.bls12_381_g2_add_cost,
            bls12_381_g1_subtract_cost: value.bls12_381_g1_subtract_cost,
            bls12_381_g2_subtract_cost: value.bls12_381_g2_subtract_cost,
            bls12_381_g1_multiply_cost: value.bls12_381_g1_multiply_cost,
            bls12_381_g2_multiply_cost: value.bls12_381_g2_multiply_cost,
            bls12_381_g1_decompress_cost: value.bls12_381_g1_decompress_cost,
            bls12_381_g2_decompress_cost: value.bls12_381_g2_decompress_cost,
            bls12_381_g1_validate_cost: value.bls12_381_g1_validate_cost,
            bls12_381_g2_validate_cost: value.bls12_381_g2_validate_cost,
            bls12_381_one_pair_cost: value.bls12_381_one_pair_cost,
            bls12_381_additional_pair_cost: value.bls12_381_additional_pair_cost,
            // The wire shape predates the two modular exponentiation costs, so a restored
            // budget takes the defaults for them.
            ..Self::new_with_defaults(false)
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct InnerInstructionWire {
    pub instruction: CompiledInstruction,
    pub stack_height: u8,
}

impl From<InnerInstruction> for InnerInstructionWire {
    fn from(value: InnerInstruction) -> Self {
        Self {
            instruction: value.instruction,
            stack_height: value.stack_height,
        }
    }
}

impl From<InnerInstructionWire> for InnerInstruction {
    fn from(value: InnerInstructionWire) -> Self {
        Self {
            instruction: value.instruction,
            stack_height: value.stack_height,
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeatureActivationWire {
    pub address: Address,
    pub slot: u64,
}

impl From<(Address, u64)> for FeatureActivationWire {
    fn from((address, slot): (Address, u64)) -> Self {
        Self { address, slot }
    }
}

impl From<FeatureActivationWire> for (Address, u64) {
    fn from(entry: FeatureActivationWire) -> Self {
        (entry.address, entry.slot)
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct TransactionMetadataWire {
    pub signature: Signature,
    pub logs: Vec<String>,
    pub inner_instructions: Vec<Vec<InnerInstructionWire>>,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
    pub fee: u64,
}

impl From<TransactionMetadata> for TransactionMetadataWire {
    fn from(value: TransactionMetadata) -> Self {
        Self {
            signature: value.signature,
            logs: value.logs,
            inner_instructions: value
                .inner_instructions
                .into_iter()
                .map(|group| group.into_iter().map(Into::into).collect())
                .collect(),
            compute_units_consumed: value.compute_units_consumed,
            return_data: value.return_data,
            fee: value.fee,
        }
    }
}

impl From<TransactionMetadataWire> for TransactionMetadata {
    fn from(value: TransactionMetadataWire) -> Self {
        Self {
            signature: value.signature,
            logs: value.logs,
            inner_instructions: value
                .inner_instructions
                .into_iter()
                .map(|group| group.into_iter().map(Into::into).collect())
                .collect(),
            compute_units_consumed: value.compute_units_consumed,
            return_data: value.return_data,
            fee: value.fee,
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FailedTransactionMetadataWire {
    pub err: TransactionError,
    pub meta: TransactionMetadataWire,
}

impl From<FailedTransactionMetadata> for FailedTransactionMetadataWire {
    fn from(value: FailedTransactionMetadata) -> Self {
        Self {
            err: value.err,
            meta: value.meta.into(),
        }
    }
}

impl From<FailedTransactionMetadataWire> for FailedTransactionMetadata {
    fn from(value: FailedTransactionMetadataWire) -> Self {
        Self {
            err: value.err,
            meta: value.meta.into(),
        }
    }
}

/// Mirror of `Result<TransactionMetadata, FailedTransactionMetadata>` so
/// wincode can derive a schema for it.
#[derive(SchemaWrite, SchemaRead)]
pub(crate) enum TxResult {
    Ok(TransactionMetadataWire),
    Err(FailedTransactionMetadataWire),
}

impl TxResult {
    pub fn from_result(r: TransactionResult) -> Self {
        match r {
            Ok(m) => TxResult::Ok(m.into()),
            Err(e) => TxResult::Err(e.into()),
        }
    }

    pub fn into_result(self) -> TransactionResult {
        match self {
            TxResult::Ok(m) => Ok(m.into()),
            TxResult::Err(e) => Err(e.into()),
        }
    }
}

// ── FeatureSet snapshot (uses AHashMap/AHashSet, can't use serde remote) ──

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct FeatureSetSnapshot {
    pub active: Vec<FeatureActivationWire>,
    pub inactive: Vec<Address>,
}

impl FeatureSetSnapshot {
    pub fn from_feature_set(fs: &FeatureSet) -> Self {
        let active = fs
            .active()
            .iter()
            .map(|(k, v)| FeatureActivationWire::from((*k, *v)))
            .collect();
        let inactive = fs.inactive().iter().copied().collect();
        Self { active, inactive }
    }

    pub fn into_feature_set(self) -> FeatureSet {
        FeatureSet::new(
            self.active.into_iter().map(Into::into).collect(),
            self.inactive.into_iter().collect(),
        )
    }
}

// ── Top-level snapshot ─────────────────────────────────────────────────

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct AccountEntryWire {
    pub address: Address,
    pub account: AccountSharedData,
}

impl From<(Address, AccountSharedData)> for AccountEntryWire {
    fn from((address, account): (Address, AccountSharedData)) -> Self {
        Self { address, account }
    }
}

impl From<AccountEntryWire> for (Address, AccountSharedData) {
    fn from(entry: AccountEntryWire) -> Self {
        (entry.address, entry.account)
    }
}

#[derive(SchemaRead)]
pub(crate) struct LiteSvmSnapshotV1 {
    pub accounts: Vec<AccountEntryWire>,
    pub airdrop_kp: [u8; 64],
    pub feature_set: FeatureSetSnapshot,
    pub latest_blockhash: Hash,
    pub history: Vec<(Signature, TxResult)>,
    pub history_capacity: u64,
    pub compute_budget: Option<ComputeBudgetV1>,
    pub sigverify: bool,
    pub blockhash_check: bool,
    pub fee_structure: FeeStructureWire,
    pub log_bytes_limit: Option<u64>,
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmSnapshotV2 {
    pub accounts: Vec<AccountEntryWire>,
    pub airdrop_kp: [u8; 64],
    pub feature_set: FeatureSetSnapshot,
    pub latest_blockhash: Hash,
    pub history: Vec<(Signature, TxResult)>,
    pub history_capacity: u64,
    pub compute_budget: Option<ComputeBudgetWire>,
    pub sigverify: bool,
    pub blockhash_check: bool,
    pub fee_structure: FeeStructureWire,
    pub log_bytes_limit: Option<u64>,
}

impl From<LiteSvmSnapshotV1> for LiteSvmSnapshotV2 {
    fn from(value: LiteSvmSnapshotV1) -> Self {
        Self {
            accounts: value.accounts,
            airdrop_kp: value.airdrop_kp,
            feature_set: value.feature_set,
            latest_blockhash: value.latest_blockhash,
            history: value.history,
            history_capacity: value.history_capacity,
            compute_budget: value
                .compute_budget
                .map(|budget| ComputeBudgetWire::from(ComputeBudget::from(budget))),
            sigverify: value.sigverify,
            blockhash_check: value.blockhash_check,
            fee_structure: value.fee_structure,
            log_bytes_limit: value.log_bytes_limit,
        }
    }
}

#[derive(SchemaWrite, SchemaRead)]
pub(crate) struct LiteSvmSnapshotV3 {
    pub state: LiteSvmSnapshotV2,
    pub epoch_vote_stakes: Vec<(Address, u64)>,
}

impl From<LiteSvmSnapshotV2> for LiteSvmSnapshotV3 {
    fn from(state: LiteSvmSnapshotV2) -> Self {
        Self {
            state,
            epoch_vote_stakes: Vec::new(),
        }
    }
}
