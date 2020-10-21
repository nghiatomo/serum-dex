use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    is_mega: bool,
    is_delegate: bool,
) -> Result<(), RegistryError> {
    info!("handler: stake");

    let acc_infos = &mut accounts.iter();

    // Lockup whitelist relay itnerface.

    let depositor_tok_acc_info = next_account_info(acc_infos)?;
    // TODO: THE POOL DESTINATION VAULT HERE.
    let _pool_vault_acc_info = next_account_info(acc_infos)?;
    let depositor_tok_owner_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    // Program specific.

    let member_acc_info = next_account_info(acc_infos)?;
    let member_authority_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    // TODO: STAKING POOL ACCOUNTS HERE.

    Entity::unpack_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            Member::unpack_mut(
                &mut member_acc_info.try_borrow_mut_data()?,
                &mut |member: &mut Member| {
                    // Activation status may have changed from
                    // PendingDeactivation -> Inactive since the last transaction.
                    // So update.
                    let clock = access_control::clock(&clock_acc_info)?;
                    let registrar = access_control::registrar(&registrar_acc_info, program_id)?;
                    entity.transition_activation_if_needed(&registrar, &clock);

                    access_control(AccessControlRequest {
                        depositor_tok_owner_acc_info,
                        depositor_tok_acc_info,
                        member_acc_info,
                        registrar_acc_info,
                        member_authority_acc_info,
                        entity_acc_info,
                        token_program_acc_info,
                        amount,
                        is_mega,
                        is_delegate,
                        entity,
                        member,
                        program_id,
                    })?;

                    state_transition(StateTransitionRequest {
                        entity,
                        member,
                        amount,
                        is_delegate,
                        is_mega,
                        registrar,
                        clock,
                        depositor_tok_owner_acc_info,
                        depositor_tok_acc_info,
                        member_acc_info,
                        member_authority_acc_info,
                        entity_acc_info,
                        token_program_acc_info,
                    })
                    .map_err(Into::into)
                },
            )
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: stake");

    let AccessControlRequest {
        depositor_tok_owner_acc_info,
        depositor_tok_acc_info,
        member_acc_info,
        member_authority_acc_info,
        entity_acc_info,
        token_program_acc_info,
        registrar_acc_info,
        amount,
        is_mega,
        is_delegate,
        entity,
        member,
        program_id,
    } = req;

    // Beneficiary (or delegate) authorization.
    if !member_authority_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _ = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let member = access_control::member(
        member_acc_info,
        entity_acc_info,
        member_authority_acc_info,
        is_delegate,
        program_id,
    )?;
    // TODO: add pools here.

    // All stake from a previous generation must be withdrawn before adding
    // stake for a new generation.
    //
    // Does not include stake-intent.
    if member.generation != entity.generation {
        if !member.stake_is_empty() {
            return Err(RegistryErrorCode::StaleStakeNeedsWithdrawal)?;
        }
    }
    // Only activated nodes can stake. If this amount puts us over the
    // activation threshold then allow it, since the node will be activated
    // once the funds are staked.
    if amount + entity.activation_amount() < registrar.reward_activation_threshold {
        return Err(RegistryErrorCode::EntityNotActivated)?;
    }

    if amount > member.stake_intent(is_mega, is_delegate) {
        return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        member,
        amount,
        is_mega,
        is_delegate,
        depositor_tok_owner_acc_info,
        depositor_tok_acc_info,
        member_acc_info,
        member_authority_acc_info,
        entity_acc_info,
        token_program_acc_info,
        registrar,
        clock,
    } = req;

    // Transfer funds into the staking pool, issuing a staking pool token.
    {
        // todo
    }

    // Perform transfer in accounts for bookeeping.
    {
        member.sub_stake_intent(amount, is_mega, is_delegate);
        entity.sub_stake_intent(amount, is_mega, &registrar, &clock);

        member.add_stake(amount, is_mega, is_delegate);
        entity.add_stake(amount, is_mega, &registrar, &clock);
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    depositor_tok_owner_acc_info: &'a AccountInfo<'a>,
    depositor_tok_acc_info: &'a AccountInfo<'a>,
    member_acc_info: &'a AccountInfo<'a>,
    member_authority_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    registrar_acc_info: &'a AccountInfo<'a>,
    is_mega: bool,
    is_delegate: bool,
    amount: u64,
    entity: &'b Entity,
    member: &'b Member,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a, 'b> {
    entity: &'b mut Entity,
    member: &'b mut Member,
    registrar: Registrar,
    clock: Clock,
    amount: u64,
    is_mega: bool,
    is_delegate: bool,
    depositor_tok_owner_acc_info: &'a AccountInfo<'a>,
    depositor_tok_acc_info: &'a AccountInfo<'a>,
    member_acc_info: &'a AccountInfo<'a>,
    member_authority_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}
