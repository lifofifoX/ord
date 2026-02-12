use super::*;

fn script_pubkey_is_address(index: &Index, script_pubkey: &[u8]) -> bool {
  let script_pubkey_buf = ScriptBuf::from_bytes(script_pubkey.to_vec());
  index
    .settings
    .chain()
    .address_from_script(&script_pubkey_buf)
    .is_ok()
}

pub(super) fn index_transfer_history_event(
  index: &Index,
  block_height: u32,
  next_transfer_number: &mut u64,
  sequence_number_to_transfer_number: &mut MultimapTable<'_, u32, u64>,
  script_pubkey_to_transfer_number: &mut MultimapTable<'_, &'static [u8], u64>,
  transfer_number_to_event: &mut Table<'_, u64, InscriptionTransferEventValue>,
  sequence_number: u32,
  sender_script_pubkey: Option<&[u8]>,
  destination_script_pubkey: Option<&[u8]>,
  op_return: bool,
) -> Result {
  if !index.index_addresses {
    return Ok(());
  }

  let sender_script_pubkey = sender_script_pubkey
    .filter(|script_pubkey| !script_pubkey.is_empty())
    .map(<[u8]>::to_vec);

  let destination_script_pubkey = destination_script_pubkey
    .filter(|script_pubkey| !script_pubkey.is_empty())
    .filter(|_| !op_return)
    .map(<[u8]>::to_vec);

  let sender_address_script_pubkey = sender_script_pubkey
    .as_deref()
    .filter(|script_pubkey| script_pubkey_is_address(index, script_pubkey));
  let destination_address_script_pubkey = destination_script_pubkey
    .as_deref()
    .filter(|script_pubkey| script_pubkey_is_address(index, script_pubkey));

  let transfer_number = *next_transfer_number;

  transfer_number_to_event.insert(
    &transfer_number,
    &InscriptionTransferEvent {
      block_height,
      sequence_number,
      from_script_pubkey: sender_script_pubkey.clone().unwrap_or_default(),
      to_script_pubkey: destination_script_pubkey.clone(),
    }
    .store(),
  )?;

  sequence_number_to_transfer_number.insert(&sequence_number, &transfer_number)?;

  if let Some(sender_script_pubkey) = sender_address_script_pubkey {
    script_pubkey_to_transfer_number.insert(sender_script_pubkey, &transfer_number)?;
  }

  if let Some(destination_script_pubkey) = destination_address_script_pubkey
    && sender_address_script_pubkey != Some(destination_script_pubkey)
  {
    script_pubkey_to_transfer_number.insert(destination_script_pubkey, &transfer_number)?;
  }

  *next_transfer_number += 1;

  Ok(())
}
