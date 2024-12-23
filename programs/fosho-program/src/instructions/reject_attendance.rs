use crate::{constant::*, error::FoshoErrors, state::*, utils::check_if_already_scanned};
use anchor_lang::prelude::*;

use mpl_core::{
  accounts::{BaseAssetV1, BaseCollectionV1},
  instructions::{UpdatePluginV1CpiBuilder, WriteExternalPluginAdapterDataV1CpiBuilder},
  types::{
    ExternalPluginAdapterKey, PermanentFreezeDelegate, Plugin, PluginAuthority, UpdateAuthority,
  },
  ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct RejectAttendee<'info> {
  #[account(
    mut,
    seeds = [
      ATTENDEE_PRE_SEED.as_ref(),
      event.key().as_ref(),
      attendee_record.owner.key().as_ref(),
    ],
    bump= attendee_record.bump,
    has_one = event,
  )]
  pub attendee_record: Box<Account<'info, Attendee>>,
  #[account(
    seeds = [
      EVENT_PRE_SEED.as_ref(),
      community.key().as_ref(),
      &event.nonce.to_le_bytes()
    ],
    bump = event.bump,
    has_one = community,
  )]
  pub event: Box<Account<'info, Event>>,
  #[account(
    seeds = [
      COMMUNITY_PRE_SEED.as_ref(),
      community.seed.as_ref(),
    ],
    bump = community.bump,
  )]
  pub community: Box<Account<'info, Community>>,
  #[account(
      mut,
      seeds = [
        EVENT_PRE_SEED.as_ref(),
        event.key().as_ref(),
        EVENT_COLLECTION_SUFFIX_SEED.as_ref(),
      ],
      bump,
      constraint = event_collection.update_authority == community.key(),
  )]
  pub event_collection: Box<Account<'info, BaseCollectionV1>>,
  #[account(
      mut,
      seeds = [
        EVENT_PRE_SEED.as_ref(),
        event.key().as_ref(),
        attendee_record.owner.key().as_ref(),
        TICKET_SUFFIX_SEED.as_ref(),
      ],
      bump,
      constraint = ticket.owner == owner.key(),
      constraint = ticket.update_authority == UpdateAuthority::Collection(event_collection.key()),
  )]
  pub ticket: Box<Account<'info, BaseAssetV1>>,
  pub system_program: Program<'info, System>,
  /// CHECK: This is checked by the ticket constraint
  pub owner: AccountInfo<'info>,
  #[account(mut)]
  pub event_authority: Signer<'info>,
  /// CHECK: This is checked by the address constraint
  #[account(address = MPL_CORE_ID)]
  pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> RejectAttendee<'info> {
  pub fn scan_ticket(&self) -> Result<()> {
    // check if community authority scanned
    check_if_already_scanned(self.ticket.to_account_info(), &self.community.key())?;

    let data: Vec<u8> = "Rejected".as_bytes().to_vec();

    let signer_seeds = &[
      COMMUNITY_PRE_SEED.as_ref(),
      self.community.seed.as_ref(),
      &[self.community.bump],
    ];

    // The event authority is the `signer` of this instruction.
    WriteExternalPluginAdapterDataV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
      .asset(&self.ticket.to_account_info())
      .collection(Some(&self.event_collection.to_account_info()))
      .payer(&self.event_authority.to_account_info())
      .authority(Some(&self.community.to_account_info()))
      .system_program(&self.system_program.to_account_info())
      .key(ExternalPluginAdapterKey::AppData(
        PluginAuthority::Address {
          address: self.community.key(),
        },
      ))
      .data(data)
      .invoke_signed(&[signer_seeds])?;

    UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
      .asset(&self.ticket.to_account_info())
      .collection(Some(&self.event_collection.to_account_info()))
      .payer(&self.event_authority.to_account_info())
      .authority(Some(&self.community.to_account_info()))
      .system_program(&self.system_program.to_account_info())
      .plugin(Plugin::PermanentFreezeDelegate(PermanentFreezeDelegate {
        frozen: true,
      }))
      .invoke_signed(&[signer_seeds])?;

    Ok(())
  }
}

pub fn reject_attendee_handler(ctx: Context<RejectAttendee>) -> Result<()> {
  let attendee_record = &mut ctx.accounts.attendee_record;
  let event = &ctx.accounts.event;

  match attendee_record.status {
    AttendeeStatus::Pending => {}
    AttendeeStatus::Claimed => {
      return Err(FoshoErrors::AlreadyClaimed.into());
    }
    AttendeeStatus::Rejected => {
      return Err(FoshoErrors::AlreadyScanned.into());
    }
    AttendeeStatus::Verified => {
      return Err(FoshoErrors::AlreadyScanned.into());
    }
  }

  let is_community_authority =
    ctx.accounts.event_authority.key() == ctx.accounts.community.authority;
  if !is_community_authority {
    require!(
      event
        .event_authorities
        .contains(&ctx.accounts.event_authority.key()),
      FoshoErrors::InvalidEventAuthority
    );
  }
  attendee_record.status = AttendeeStatus::Rejected;

  ctx.accounts.scan_ticket()?;
  Ok(())
}
