use super::*;
use redb::{Database, TableDefinition};

#[derive(serde::Deserialize)]
struct CleanupOutput {
  rows_before: u64,
  rows_after: u64,
  table_deleted: bool,
}

#[test]
fn run_is_an_alias_for_update() {
  let core = mockcore::spawn();
  core.mine_blocks(1);

  let tempdir = TempDir::new().unwrap();

  let index_path = tempdir.path().join("foo.redb");

  CommandBuilder::new(format!("--index {} index run", index_path.display()))
    .core(&core)
    .run_and_extract_stdout();

  assert!(index_path.is_file())
}

#[test]
fn custom_index_path() {
  let core = mockcore::spawn();
  core.mine_blocks(1);

  let tempdir = TempDir::new().unwrap();

  let index_path = tempdir.path().join("foo.redb");

  CommandBuilder::new(format!("--index {} index update", index_path.display()))
    .core(&core)
    .run_and_extract_stdout();

  assert!(index_path.is_file())
}

#[test]
fn re_opening_database_does_not_trigger_schema_check() {
  let core = mockcore::spawn();
  core.mine_blocks(1);

  let tempdir = TempDir::new().unwrap();

  let index_path = tempdir.path().join("foo.redb");

  CommandBuilder::new(format!("--index {} index update", index_path.display()))
    .core(&core)
    .run_and_extract_stdout();

  assert!(index_path.is_file());

  CommandBuilder::new(format!("--index {} index update", index_path.display()))
    .core(&core)
    .run_and_extract_stdout();
}

#[test]
fn export_inscription_number_to_id_tsv() {
  let core = mockcore::spawn();
  let ord = TestServer::spawn(&core);

  create_wallet(&core, &ord);

  let temp_dir = TempDir::new().unwrap();

  inscribe(&core, &ord);
  inscribe(&core, &ord);

  let (inscription, _) = inscribe(&core, &ord);

  core.mine_blocks(1);

  let tsv = CommandBuilder::new("index export --tsv foo.tsv")
    .core(&core)
    .temp_dir(Arc::new(temp_dir))
    .run_and_extract_file("foo.tsv");

  let entries: BTreeMap<i64, ord::Object> = tsv
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
    .map(|line| {
      let value = line.split('\t').collect::<Vec<&str>>();
      let inscription_number = i64::from_str(value[0]).unwrap();
      let inscription_id = ord::Object::from_str(value[1]).unwrap();

      (inscription_number, inscription_id)
    })
    .collect();

  assert_eq!(
    entries.get(&2).unwrap(),
    &ord::Object::InscriptionId(inscription),
  );
}

#[test]
fn cleanup_pre_jubilee_filtered_shadow_data_is_idempotent() {
  let core = mockcore::spawn();
  core.mine_blocks(1);

  let tempdir = TempDir::new().unwrap();

  let index_path = tempdir.path().join("foo.redb");

  CommandBuilder::new(format!("--index {} index update", index_path.display()))
    .core(&core)
    .run_and_extract_stdout();

  const OUTPOINT_TO_FILTERED_INSCRIPTION_DATA: TableDefinition<&[u8; 36], &[u8]> =
    TableDefinition::new("OUTPOINT_TO_FILTERED_INSCRIPTION_DATA");

  let database = Database::builder().open(&index_path).unwrap();
  let wtx = database.begin_write().unwrap();
  wtx
    .open_table(OUTPOINT_TO_FILTERED_INSCRIPTION_DATA)
    .unwrap()
    .insert(&[0; 36], &[1_u8, 2, 3][..])
    .unwrap();
  wtx.commit().unwrap();
  drop(database);

  let first_cleanup = CommandBuilder::new(format!(
    "--index {} index cleanup --pre-jubilee-filtered",
    index_path.display()
  ))
  .core(&core)
  .run_and_deserialize_output::<CleanupOutput>();

  assert!(first_cleanup.table_deleted);
  assert!(first_cleanup.rows_before > 0);
  assert_eq!(first_cleanup.rows_after, 0);

  let second_cleanup = CommandBuilder::new(format!(
    "--index {} index cleanup --pre-jubilee-filtered",
    index_path.display()
  ))
  .core(&core)
  .run_and_deserialize_output::<CleanupOutput>();

  assert!(second_cleanup.table_deleted);
  assert_eq!(second_cleanup.rows_before, 0);
  assert_eq!(second_cleanup.rows_after, 0);
}
