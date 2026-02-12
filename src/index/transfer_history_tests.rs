use {super::*, crate::index::testing::Context};

#[test]
fn inscription_transfer_history_records_and_orders_newest_first() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let inscription_id = InscriptionId {
    txid: create_txid,
    index: 0,
  };

  let transfer_1_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let transfer_2_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(4, 1, 0, Witness::new())],
    outputs: 1,
    p2tr: true,
    ..default()
  });

  context.mine_blocks(1);

  let (history, more) = context
    .index
    .get_inscription_transfer_history_paginated(inscription_id, 10, 0)
    .unwrap();

  let create_transaction = context.index.get_transaction(create_txid).unwrap().unwrap();
  let transfer_1_transaction = context
    .index
    .get_transaction(transfer_1_txid)
    .unwrap()
    .unwrap();
  let transfer_2_transaction = context
    .index
    .get_transaction(transfer_2_txid)
    .unwrap()
    .unwrap();

  let create_address = context
    .index
    .settings
    .chain()
    .address_from_script(&create_transaction.output[0].script_pubkey)
    .unwrap()
    .to_string();
  let transfer_1_address = context
    .index
    .settings
    .chain()
    .address_from_script(&transfer_1_transaction.output[0].script_pubkey)
    .unwrap()
    .to_string();
  let transfer_2_address = context
    .index
    .settings
    .chain()
    .address_from_script(&transfer_2_transaction.output[0].script_pubkey)
    .unwrap()
    .to_string();

  assert!(!more);
  assert_eq!(history.len(), 3);

  assert_eq!(history[0].inscription_id, inscription_id);
  assert_eq!(history[0].from_address, Some(transfer_1_address.clone()));
  assert_eq!(history[0].to_address, Some(transfer_2_address));

  assert_eq!(history[1].inscription_id, inscription_id);
  assert_eq!(history[1].from_address, Some(create_address.clone()));
  assert_eq!(history[1].to_address, Some(transfer_1_address));

  assert_eq!(history[2].inscription_id, inscription_id);
  assert_eq!(history[2].from_address, None);
  assert_eq!(history[2].to_address, Some(create_address));
}

#[test]
fn address_transfer_history_tracks_send_and_receive_for_standard_transfer() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let inscription_id = InscriptionId {
    txid: create_txid,
    index: 0,
  };

  let transfer_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    p2tr: true,
    ..default()
  });

  context.mine_blocks(1);

  let create_transaction = context.index.get_transaction(create_txid).unwrap().unwrap();
  let transfer_transaction = context
    .index
    .get_transaction(transfer_txid)
    .unwrap()
    .unwrap();

  let sender_address = context
    .index
    .settings
    .chain()
    .address_from_script(&create_transaction.output[0].script_pubkey)
    .unwrap();

  let receiver_address = context
    .index
    .settings
    .chain()
    .address_from_script(&transfer_transaction.output[0].script_pubkey)
    .unwrap();

  let (sender_history, sender_more) = context
    .index
    .get_address_transfer_history_paginated(&sender_address, 10, 0)
    .unwrap()
    .unwrap();

  assert!(!sender_more);
  assert_eq!(sender_history.len(), 2);
  assert_eq!(sender_history[0].inscription_id, inscription_id);
  assert_eq!(
    sender_history[0].from_address,
    Some(sender_address.to_string())
  );
  assert_eq!(
    sender_history[0].to_address,
    Some(receiver_address.to_string())
  );
  assert_eq!(sender_history[1].inscription_id, inscription_id);
  assert_eq!(sender_history[1].from_address, None);
  assert_eq!(
    sender_history[1].to_address,
    Some(sender_address.to_string())
  );

  let (receiver_history, receiver_more) = context
    .index
    .get_address_transfer_history_paginated(&receiver_address, 10, 0)
    .unwrap()
    .unwrap();

  assert!(!receiver_more);
  assert_eq!(receiver_history.len(), 1);
  assert_eq!(receiver_history[0], sender_history[0]);
}

#[test]
fn address_transfer_history_tracks_burn_sender_only_for_op_return() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let transfer_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 0,
    op_return_index: Some(0),
    op_return_value: Some(50 * COIN_VALUE),
    op_return: Some(
      script::Builder::new()
        .push_opcode(opcodes::all::OP_RETURN)
        .into_script(),
    ),
    ..default()
  });

  context.mine_blocks(1);

  let create_transaction = context.index.get_transaction(create_txid).unwrap().unwrap();
  let sender_address = context
    .index
    .settings
    .chain()
    .address_from_script(&create_transaction.output[0].script_pubkey)
    .unwrap();

  let (sender_history, sender_more) = context
    .index
    .get_address_transfer_history_paginated(&sender_address, 10, 0)
    .unwrap()
    .unwrap();

  assert!(!sender_more);
  assert_eq!(sender_history.len(), 2);
  assert_eq!(
    sender_history[0].from_address,
    Some(sender_address.to_string())
  );
  assert_eq!(sender_history[0].to_address, None);
  assert_eq!(sender_history[1].from_address, None);
  assert_eq!(
    sender_history[1].to_address,
    Some(sender_address.to_string())
  );

  let transfer_transaction = context
    .index
    .get_transaction(transfer_txid)
    .unwrap()
    .unwrap();
  let rtx = context.index.database.begin_read().unwrap();
  let script_pubkey_to_transfer_number = rtx
    .open_multimap_table(SCRIPT_PUBKEY_TO_TRANSFER_NUMBER)
    .unwrap();

  assert_eq!(
    script_pubkey_to_transfer_number
      .get(transfer_transaction.output[0].script_pubkey.as_bytes())
      .unwrap()
      .count(),
    0
  );
}

#[test]
fn spendable_non_address_scripts_are_persisted_in_transfer_events() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let non_address_script = script::Builder::new()
    .push_opcode(opcodes::all::OP_PUSHNUM_1)
    .into_script();

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 0,
    op_return_index: Some(0),
    op_return_value: Some(50 * COIN_VALUE),
    op_return: Some(non_address_script.clone()),
    ..default()
  });

  context.mine_blocks(1);

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(4, 1, 0, Witness::new())],
    outputs: 1,
    p2tr: true,
    ..default()
  });

  context.mine_blocks(1);

  let inscription_id = InscriptionId {
    txid: create_txid,
    index: 0,
  };

  let rtx = context.index.database.begin_read().unwrap();
  let inscription_id_to_sequence_number =
    rtx.open_table(INSCRIPTION_ID_TO_SEQUENCE_NUMBER).unwrap();
  let sequence_number_to_transfer_number = rtx
    .open_multimap_table(SEQUENCE_NUMBER_TO_TRANSFER_NUMBER)
    .unwrap();
  let transfer_number_to_event = rtx.open_table(TRANSFER_NUMBER_TO_EVENT).unwrap();
  let script_pubkey_to_transfer_number = rtx
    .open_multimap_table(SCRIPT_PUBKEY_TO_TRANSFER_NUMBER)
    .unwrap();

  let sequence_number = inscription_id_to_sequence_number
    .get(&inscription_id.store())
    .unwrap()
    .unwrap()
    .value();

  let events = sequence_number_to_transfer_number
    .get(sequence_number)
    .unwrap()
    .map(|result| {
      let transfer_number = result.unwrap().value();
      let event = transfer_number_to_event
        .get(&transfer_number)
        .unwrap()
        .unwrap();
      InscriptionTransferEvent::load(event.value())
    })
    .collect::<Vec<_>>();

  assert_eq!(events.len(), 3);
  assert!(
    events
      .iter()
      .any(|event| { event.to_script_pubkey.as_deref() == Some(non_address_script.as_bytes()) })
  );
  assert!(
    events
      .iter()
      .any(|event| { event.from_script_pubkey.as_slice() == non_address_script.as_bytes() })
  );
  assert_eq!(
    script_pubkey_to_transfer_number
      .get(non_address_script.as_bytes())
      .unwrap()
      .count(),
    0
  );
}

#[test]
fn address_transfer_history_requires_address_index() {
  let context = Context::builder().build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let transfer_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let transfer_transaction = context
    .index
    .get_transaction(transfer_txid)
    .unwrap()
    .unwrap();
  let address = context
    .index
    .settings
    .chain()
    .address_from_script(&transfer_transaction.output[0].script_pubkey)
    .unwrap();

  assert_eq!(
    context
      .index
      .get_address_transfer_history_paginated(&address, 10, 0)
      .unwrap(),
    None
  );
  assert_eq!(
    context
      .index
      .get_inscription_transfer_history_paginated(
        InscriptionId {
          txid: create_txid,
          index: 0,
        },
        10,
        0,
      )
      .unwrap()
      .0
      .len(),
    0
  );

  assert_eq!(
    context
      .index
      .get_transfer_history_in_block(4)
      .unwrap()
      .len(),
    0
  );
}

#[test]
fn transfer_history_pagination_more_flag() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let inscription_id = InscriptionId {
    txid: create_txid,
    index: 0,
  };

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });
  context.mine_blocks(1);

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(4, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });
  context.mine_blocks(1);

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(5, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });
  context.mine_blocks(1);

  let (page_0, page_0_more) = context
    .index
    .get_inscription_transfer_history_paginated(inscription_id, 2, 0)
    .unwrap();

  assert!(page_0_more);
  assert_eq!(page_0.len(), 2);
  assert!(page_0[0].block_height > page_0[1].block_height);

  let (page_1, page_1_more) = context
    .index
    .get_inscription_transfer_history_paginated(inscription_id, 2, 1)
    .unwrap();

  assert!(!page_1_more);
  assert_eq!(page_1.len(), 2);
  assert!(page_1[0].block_height < page_0[1].block_height);
  assert!(page_1[1].block_height <= page_1[0].block_height);
}

#[test]
fn transfer_history_in_block_uses_height_boundaries() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let inscription_id = InscriptionId {
    txid: create_txid,
    index: 0,
  };

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(4, 1, 0, Witness::new())],
    outputs: 1,
    p2tr: true,
    ..default()
  });

  context.mine_blocks(1);

  let (history, more) = context
    .index
    .get_inscription_transfer_history_paginated(inscription_id, 10, 0)
    .unwrap();
  assert!(!more);
  assert_eq!(history.len(), 3);

  let newest_entry = history[0].clone();
  let middle_entry = history[1].clone();
  let oldest_entry = history[2].clone();

  assert!(
    context
      .index
      .get_transfer_history_in_block(oldest_entry.block_height.saturating_sub(1))
      .unwrap()
      .is_empty()
  );

  assert_eq!(
    context
      .index
      .get_transfer_history_in_block(oldest_entry.block_height)
      .unwrap(),
    vec![oldest_entry]
  );
  assert_eq!(
    context
      .index
      .get_transfer_history_in_block(middle_entry.block_height)
      .unwrap(),
    vec![middle_entry]
  );
  assert_eq!(
    context
      .index
      .get_transfer_history_in_block(newest_entry.block_height)
      .unwrap(),
    vec![newest_entry]
  );
}

#[test]
fn sender_receiver_same_script_is_deduped() {
  let context = Context::builder().arg("--index-addresses").build();

  context.mine_blocks(2);

  let create_txid = context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, inscription("text/plain", "hello").to_witness())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  context.core.broadcast_tx(TransactionTemplate {
    inputs: &[(3, 1, 0, Witness::new())],
    outputs: 1,
    ..default()
  });

  context.mine_blocks(1);

  let create_transaction = context.index.get_transaction(create_txid).unwrap().unwrap();
  let address = context
    .index
    .settings
    .chain()
    .address_from_script(&create_transaction.output[0].script_pubkey)
    .unwrap();

  let (history, more) = context
    .index
    .get_address_transfer_history_paginated(&address, 10, 0)
    .unwrap()
    .unwrap();

  assert!(!more);
  assert_eq!(history.len(), 2);
  assert_eq!(history[0].from_address, Some(address.to_string()));
  assert_eq!(history[0].to_address, Some(address.to_string()));
  assert_eq!(history[1].from_address, None);
  assert_eq!(history[1].to_address, Some(address.to_string()));
}
