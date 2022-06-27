use frame_support::weights::Weight;
use crate::Multisig;
use frame_support::traits::Get;
use crate::StorageVersion;

pub fn migrate<T: crate::Config>() -> Weight {
	if Multisig::<T>::get().is_some() {
		Multisig::<T>::take();
	}
	StorageVersion::new(2).put::<crate::Pallet<T>>();
	T::DbWeight::get().reads_writes(1, 1)
}

pub fn pre_migration<T: crate::Config>() {
	// assert_eq!(StorageVersion::get::<crate::Pallet<T>>(), 1);
}

pub fn post_migration<T: crate::Config>() {
	assert!(Multisig::<T>::get().is_none());
	assert_eq!(StorageVersion::get::<crate::Pallet<T>>(), 2);
}
