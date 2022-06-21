
#![allow(dead_code)]
use frame_support::{
	traits::{Get, StorageVersion},
	weights::Weight,
	runtime_print,
};
use log;
use sp_io;
use sp_core;

/// The old prefix.
pub const OLD_PREFIX: &[u8] = b"Listen";

/// Migrate the entire storage of this pallet to a new prefix.
///
/// This new prefix must be the same as the one set in construct_runtime. For safety, use
/// `PalletInfo` to get it, as:
/// `<Runtime as frame_system::Config>::PalletInfo::name::<ElectionsPhragmenPallet>`.
///
/// The old storage prefix, `PhragmenElection` is hardcoded in the migration code.
pub fn migrate<T: crate::Config, N: AsRef<str>>(new_pallet_name: N) -> Weight {
	if new_pallet_name.as_ref().as_bytes() == OLD_PREFIX {
		log::info!(
			target: "runtime::elections-phragmen",
			"New pallet name is equal to the old prefix. No migration needs to be done.",
		);
		return 0
	}
	let storage_version = StorageVersion::get::<crate::Pallet<T>>();
	log::info!(
		target: "runtime::elections-phragmen",
		"Running migration to v4 for elections-phragmen with storage version {:?}",
		storage_version,
	);

	if storage_version <= 0 {
		log::info!("new prefix: {}", new_pallet_name.as_ref());
		frame_support::storage::migration::move_pallet(
			OLD_PREFIX,
			new_pallet_name.as_ref().as_bytes(),
		);

		StorageVersion::new(1).put::<crate::Pallet<T>>();

		<T as frame_system::Config>::BlockWeights::get().max_block
	} else {
		log::warn!(
			target: "runtime::listen",
			"Attempted to apply migration to v4 but failed because storage version is {:?}",
			storage_version,
		);
		0
	}
}

/// Some checks prior to migration. This can be linked to
/// [`frame_support::traits::OnRuntimeUpgrade::pre_upgrade`] for further testing.
///
/// Panics if anything goes wrong.
pub fn pre_migration<T: crate::Config, N: AsRef<str>>(new: N) {
	let new = new.as_ref();
	log::info!("pre-migration listen test with new = {}", new);
	// the next key must exist, and start with the hash of `OLD_PREFIX`.
	let next_key = sp_io::storage::next_key(OLD_PREFIX).unwrap();
	runtime_print!("next_key: {:?}", next_key);
	// assert!(next_key.starts_with(&sp_io::hashing::twox_128(OLD_PREFIX)));
	//
	// // ensure nothing is stored in the new prefix.
	// assert!(
	// 	sp_io::storage::next_key(new.as_bytes()).map_or(
	// 		// either nothing is there
	// 		true,
	// 		// or we ensure that it has no common prefix with twox_128(new).
	// 		|next_key| !next_key.starts_with(&sp_io::hashing::twox_128(new.as_bytes()))
	// 	),
	// 	"unexpected next_key({}) = {:?}",
	// 	new,
	// 	sp_core::hexdisplay::HexDisplay::from(&sp_io::storage::next_key(new.as_bytes()).unwrap())
	// );
	// ensure storage version is 3.
	assert_eq!(StorageVersion::get::<crate::Pallet<T>>(), 0);
}

/// Some checks for after migration. This can be linked to
/// [`frame_support::traits::OnRuntimeUpgrade::post_upgrade`] for further testing.
///
/// Panics if anything goes wrong.
pub fn post_migration<T: crate::Config>() {
	log::info!("post-migration listen");
	// ensure we've been updated to v4 by the automatic write of crate version -> storage version.
	assert_eq!(StorageVersion::get::<crate::Pallet<T>>(), 1);
}
