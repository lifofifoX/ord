use super::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InscriptionTransferHistoryEntry {
  pub block_height: u32,
  pub inscription_id: InscriptionId,
  pub sequence_number: u32,
  pub from_address: Option<String>,
  pub to_address: Option<String>,
  pub old_satpoint: Option<SatPoint>,
  pub new_satpoint: SatPoint,
  pub spent_as_fee_in_txid: Option<Txid>,
}

impl Index {
  pub fn get_inscription_transfer_history_paginated(
    &self,
    inscription_id: InscriptionId,
    page_size: usize,
    page_index: usize,
  ) -> Result<(Vec<InscriptionTransferHistoryEntry>, bool)> {
    if !self.has_address_index() {
      return Ok((Vec::new(), false));
    }

    let rtx = self.database.begin_read()?;

    let inscription_id_to_sequence_number = rtx.open_table(INSCRIPTION_ID_TO_SEQUENCE_NUMBER)?;
    let sequence_number_to_transfer_number =
      rtx.open_multimap_table(SEQUENCE_NUMBER_TO_TRANSFER_NUMBER)?;
    let sequence_number_to_inscription_entry =
      rtx.open_table(SEQUENCE_NUMBER_TO_INSCRIPTION_ENTRY)?;
    let transfer_number_to_event = rtx.open_table(TRANSFER_NUMBER_TO_EVENT)?;

    let Some(sequence_number) = inscription_id_to_sequence_number
      .get(&inscription_id.store())?
      .map(|guard| guard.value())
    else {
      return Ok((Vec::new(), false));
    };

    let mut entries = sequence_number_to_transfer_number
      .get(sequence_number)?
      .rev()
      .skip(page_index.saturating_mul(page_size))
      .take(page_size.saturating_add(1))
      .map(|result| {
        let transfer_number = result?.value();
        self.load_transfer_history_entry(
          &transfer_number_to_event,
          &sequence_number_to_inscription_entry,
          transfer_number,
        )
      })
      .collect::<Result<Vec<InscriptionTransferHistoryEntry>>>()?;

    let more = entries.len() > page_size;

    if more {
      entries.pop();
    }

    Ok((entries, more))
  }

  pub fn get_transfer_history_in_block(
    &self,
    block_height: u32,
  ) -> Result<Vec<InscriptionTransferHistoryEntry>> {
    if !self.has_address_index() {
      return Ok(Vec::new());
    }

    let rtx = self.database.begin_read()?;

    let height_to_last_transfer_number = rtx.open_table(HEIGHT_TO_LAST_TRANSFER_NUMBER)?;
    let sequence_number_to_inscription_entry =
      rtx.open_table(SEQUENCE_NUMBER_TO_INSCRIPTION_ENTRY)?;
    let transfer_number_to_event = rtx.open_table(TRANSFER_NUMBER_TO_EVENT)?;

    let Some(newest_transfer_number) = height_to_last_transfer_number
      .get(&block_height)?
      .map(|guard| guard.value())
    else {
      return Ok(Vec::new());
    };

    let oldest_transfer_number = height_to_last_transfer_number
      .get(block_height.saturating_sub(1))?
      .map(|guard| guard.value())
      .unwrap_or(0);

    (oldest_transfer_number..newest_transfer_number)
      .map(|transfer_number| {
        self.load_transfer_history_entry(
          &transfer_number_to_event,
          &sequence_number_to_inscription_entry,
          transfer_number,
        )
      })
      .collect()
  }

  pub fn get_address_transfer_history_paginated(
    &self,
    address: &Address,
    page_size: usize,
    page_index: usize,
  ) -> Result<Option<(Vec<InscriptionTransferHistoryEntry>, bool)>> {
    if !self.has_address_index() {
      return Ok(None);
    }

    let rtx = self.database.begin_read()?;

    let script_pubkey_to_transfer_number =
      rtx.open_multimap_table(SCRIPT_PUBKEY_TO_TRANSFER_NUMBER)?;
    let sequence_number_to_inscription_entry =
      rtx.open_table(SEQUENCE_NUMBER_TO_INSCRIPTION_ENTRY)?;
    let transfer_number_to_event = rtx.open_table(TRANSFER_NUMBER_TO_EVENT)?;

    let mut entries = script_pubkey_to_transfer_number
      .get(address.script_pubkey().as_bytes())?
      .rev()
      .skip(page_index.saturating_mul(page_size))
      .take(page_size.saturating_add(1))
      .map(|result| {
        let transfer_number = result?.value();
        self.load_transfer_history_entry(
          &transfer_number_to_event,
          &sequence_number_to_inscription_entry,
          transfer_number,
        )
      })
      .collect::<Result<Vec<InscriptionTransferHistoryEntry>>>()?;

    let more = entries.len() > page_size;

    if more {
      entries.pop();
    }

    Ok(Some((entries, more)))
  }

  fn load_transfer_history_entry(
    &self,
    transfer_number_to_event: &impl ReadableTable<u64, InscriptionTransferEventValue>,
    sequence_number_to_inscription_entry: &impl ReadableTable<u32, InscriptionEntryValue>,
    transfer_number: u64,
  ) -> Result<InscriptionTransferHistoryEntry> {
    let event = transfer_number_to_event
      .get(&transfer_number)?
      .map(|entry| InscriptionTransferEvent::load(entry.value()))
      .unwrap();

    let inscription_id = sequence_number_to_inscription_entry
      .get(event.sequence_number)?
      .map(|entry| InscriptionEntry::load(entry.value()).id)
      .unwrap();

    Ok(InscriptionTransferHistoryEntry {
      block_height: event.block_height,
      inscription_id,
      sequence_number: event.sequence_number,
      from_address: self.script_pubkey_to_address(&event.from_script_pubkey),
      to_address: event
        .to_script_pubkey
        .as_deref()
        .and_then(|script_pubkey| self.script_pubkey_to_address(script_pubkey)),
      old_satpoint: event.old_satpoint,
      new_satpoint: event.new_satpoint,
      spent_as_fee_in_txid: event.spent_as_fee_in_txid,
    })
  }

  fn script_pubkey_to_address(&self, script_pubkey: &[u8]) -> Option<String> {
    self
      .settings
      .chain()
      .address_from_script(&ScriptBuf::from_bytes(script_pubkey.to_vec()))
      .ok()
      .map(|address| address.to_string())
  }
}
