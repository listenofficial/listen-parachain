use super::*;

pub struct RemoveListenToRoom;

impl frame_support::traits::OnRuntimeUpgrade for RemoveListenToRoom {
	fn on_runtime_upgrade() -> Weight {
		let new_pallet_name = <Runtime as frame_system::Config>::PalletInfo::name::<Room>()
   .expect("Bounties is part of runtime, so it has a name; qed");
		pallet_room::migrations::v1::migrate::<Runtime, _>(new_pallet_name)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		let new_pallet_name = <Runtime as frame_system::Config>::PalletInfo::name::<Room>()
   .expect("Bounties is part of runtime, so it has a name; qed");
		pallet_room::migrations::v1::pre_migration::<Runtime, _>(new_pallet_name);
		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		pallet_room::migrations::v1::post_migration::<Runtime>();
		assert!(Room::server_id().is_some());
		Ok(())
	}

}

pub struct OnRuntimeUpgrade;
impl frame_support::traits::OnRuntimeUpgrade for OnRuntimeUpgrade {
	fn on_runtime_upgrade() -> u64 {
		frame_support::migrations::migrate_from_pallet_version_to_storage_version::<
			AllPalletsWithSystem,
		>(&RocksDbWeight::get())
	}
}
