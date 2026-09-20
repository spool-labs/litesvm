use {
    solana_program_runtime::invoke_context::InvokeContext, solana_svm_timings::ExecuteTimings,
    solana_svm_transaction::svm_message::SVMMessage, solana_transaction_context::IndexOfAccount,
    solana_transaction_error::TransactionError,
};

/// Process a message.
/// This method calls each instruction in the message over the set of loaded accounts.
/// For each instruction it calls the program entrypoint method and verifies that the result of
/// the call does not violate the bank's accounting rules.
/// The accounts are committed back to the bank only if every instruction succeeds.
pub(crate) fn process_message<'ix_data>(
    message: &'ix_data impl SVMMessage,
    program_indices: &[IndexOfAccount],
    invoke_context: &mut InvokeContext<'_, 'ix_data>,
    execute_timings: &mut ExecuteTimings,
    accumulated_consumed_units: &mut u64,
) -> Result<(), TransactionError> {
    debug_assert_eq!(program_indices.len(), message.num_instructions());
    invoke_context
        .process_message(message, execute_timings, accumulated_consumed_units)
        .map_err(|(instruction_index, err)| {
            TransactionError::InstructionError(instruction_index, err)
        })
}
