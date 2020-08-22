#![cfg_attr(not(feature = "std"), no_std)]
use codec::alloc::string::ToString;
use core::convert::TryInto;
// use rstd::prelude::*;
use sp_core::crypto::KeyTypeId;
use system::offchain;
use codec::{Encode, Decode};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, dispatch, ensure,
	sp_runtime::{
		RuntimeDebug,
		DispatchError,
		offchain::{
        self as rt_offchain,
        storage::StorageValueRef,
		storage_lock::{
			StorageLock, Time
		}
		},
		transaction_validity::{
			TransactionValidity, TransactionLongevity, ValidTransaction, InvalidTransaction, TransactionPriority
		  },
		traits::{AtLeast32Bit, Bounded, Member, Hash},
	},
	traits::{Get, EnsureOrigin},
	sp_std::{
		prelude::*,
		str,
		vec::Vec,
		fmt,
	},
};
use frame_system::{
	self as system, ensure_signed, 
	offchain::{
		AppCrypto, CreateSignedTransaction, SendUnsignedTransaction, 
		SignedPayload, SigningTypes, Signer, SubmitTransaction, 
		SendTransactionTypes, SendSignedTransaction
	}
};

use uuid::Uuid;


#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const LOCK_TIMEOUT_EXPIRATION: u64 = 3000; // in milli-seconds
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ofcw");

use exchangable_id::{ExchangableId,UUID, ExchangableIdProvider};

pub mod crypto {
	use super::KEY_TYPE;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	use sp_core::sr25519::Signature as Sr25519Signature;
	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Contact<T: Trait, Moment> {
    pub id: ExchangableId<T>,
    pub contact_id: ExchangableId<T>,
    pub timestamp: Moment,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum FlagType {
	Positive,
	Negative,
	Suspicious
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Flag<T: Trait, Moment> {
	pub id: ExchangableId<T>,
	pub flag_type: Option<FlagType>,
	pub timestamp: Moment,
}

pub trait Trait: system::Trait + timestamp::Trait +  CreateSignedTransaction<Call<Self>> {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type ExchangableIdProvider: ExchangableIdProvider<Self>;
	type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
	type Call: From<Call<Self>>;
	type UnsignedPriority: Get<TransactionPriority>;
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq, Debug))]
#[derive(Encode, Decode)]
pub enum OffchainRequest<T: system::Trait> {
	// Ping(u8,  <T as system::Trait>::AccountId),
	FlagID(ExchangableId<T>),
	AddToUUIDPool(UUID),
}


decl_storage! {
	trait Store for Module<T: Trait> as ContactTracing {
		pub Contacts get(fn contacts): double_map hasher(blake2_128_concat) ExchangableId<T>, hasher(blake2_128_concat) ExchangableId<T> => Option<Contact<T, T::Moment>>;
		pub Flags get(fn flags): map hasher(blake2_128_concat) ExchangableId<T> => Option<Flag<T, T::Moment>>;
		OffchainRequests get(fn offchain_requests): Vec<OffchainRequest<T>>;
	}
}

decl_event!(
	pub enum Event<T> where 
		//AccountId = <T as frame_system::Trait>::AccountId,
		UUID = Vec<u8>,
		ExchangableId = <T as system::Trait>::Hash,
		FlagType = FlagType,
	{
		ContactAdded(ExchangableId),
		ContactFlagged(UUID, FlagType),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Error names should be descriptive.
		NotOwner,
		InvalidUUID,
		NoneValue,
		StorageOverflow,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		#[weight = 0]
		pub fn generate_uuid(origin, id: ExchangableId<T>) {
			let sender = ensure_signed(origin)?;
			// ensure!(
			// 	T::ExchangableIdProvider::check_uuid_exists(&id),
			// 	Error::<T>::InvalidUUID,
			// );
			// Implement Check owner
			let uuid = T::ExchangableIdProvider::get_uuid(id);
			<Self as Store>::OffchainRequests::mutate(|v| v.push(OffchainRequest::AddToUUIDPool(uuid.unwrap())));

			
		}

		#[weight = 0]
		pub fn add_contact(origin, id: UUID, contact_id: UUID) {
			let sender = ensure_signed(origin)?;
			ensure!(
				T::ExchangableIdProvider::check_uuid_exists(&id),
				Error::<T>::InvalidUUID,
			);
			ensure!(
				T::ExchangableIdProvider::check_uuid_exists(&contact_id),
				Error::<T>::InvalidUUID,
        	);
			let id = T::Hashing::hash_of(&Uuid::parse_str(str::from_utf8(&id).unwrap()).unwrap().as_bytes());
			let contact_id = T::Hashing::hash_of(&Uuid::parse_str(str::from_utf8(&contact_id).unwrap()).unwrap().as_bytes());
			let contact = Self::new_contact(id, contact_id);
			// let uuid = T::ExchangableIdProvider::get_uuid(id);
			if Self::check_contact_flag(contact_id) {
				<Self as Store>::OffchainRequests::mutate(|v| v.push(OffchainRequest::FlagID(id)));
			} 
			//check if added contact is corona positive
			
			//ensure!();
			// Self::deposit_event(RawEvent::ContactAdded(contact.id));
		}

		#[weight = 0]
		pub fn check_uuid(origin, uuid: UUID) {
			ensure!(
				T::ExchangableIdProvider::check_uuid_exists(&uuid),
				Error::<T>::InvalidUUID,
        	);
		}

		#[weight = 0]
		pub fn add_flag(origin, id: UUID, flag_type: FlagType) {
			let sender = ensure_signed(origin)?;
			let id = T::Hashing::hash_of(&Uuid::parse_str(str::from_utf8(&id).unwrap()).unwrap().as_bytes());

			// Needed fix
			// ensure!(
			// 	T::ExchangableIdProvider::check_exchangable_id_owner(sender, id),
			// 	Error::<T>::NotOwner,
			// );
			
			let flag_type_event = flag_type.clone();
			Self::new_flag(id, flag_type);
			//
			let uuid = T::ExchangableIdProvider::get_uuid(id);
			Self::deposit_event(RawEvent::ContactFlagged(uuid.unwrap(), flag_type_event));
		}

		fn offchain_worker(block_number: T::BlockNumber) {

			//debug::info!("offchain worker");
			let block_hash = <system::Module<T>>::block_hash(block_number);
			//debug::warn!("Current block is: {:?} (parent: {:?})", block_number, block_hash);

            // Acquiring the lock
            let mut lock = StorageLock::<Time>::with_deadline(
                b"contact_tracing_ocw::lock",
                rt_offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION)
            );

            match lock.try_lock() {
                Ok(_guard) => { Self::process_ocw_notifications(block_number); }
                Err(_err) => { debug::info!("[contact_tracing_ocw] lock is already acquired"); }
            };
		}
		
	}
}

impl<T: Trait> Module<T> {

	fn check_contact_flag(id: ExchangableId<T>) -> bool {
		match Flags::<T>::get(id) {
			None => false,
			Some(flag) => if flag.flag_type == Some(FlagType::Positive) || flag.flag_type == Some(FlagType::Suspicious) {
				true
			} else { false },
		}
		// let flag = Flags::<T>::get(id);
		// debug::info!("{:?}", flag);

	}

	fn new_contact(id: ExchangableId<T>, contact_id: ExchangableId<T>) -> Contact<T, T::Moment>{
		let contact = Contact::<T, T::Moment> {
			id: id,
			contact_id: contact_id,
			timestamp: <timestamp::Module<T>>::now(),
		};
		Contacts::insert(id, contact_id, &contact);
		contact
	}

	fn new_flag(id: ExchangableId<T>, flag_type: FlagType) {
		let flag = Flag::<T, T::Moment> {
			id: id,
			flag_type: Some(flag_type),
			timestamp: <timestamp::Module<T>>::now(),
		};
		Flags::insert(id,&flag);
	}
	fn check_flag(id: ExchangableId<T>){
		debug::info!("{:?}", id);
	}

	fn flag_id(id: ExchangableId<T>) {

	}

	pub fn check_exchangable_id_exists(props: &ExchangableId<T>) -> Result<(), Error<T>> {
        // ensure!(
        //     props.len() <= SHIPMENT_MAX_PRODUCTS,
        //     Error::<T>::ShipmentHasTooManyProducts,
        // );
        Ok(())
    }
	
	fn process_ocw_notifications(block_number: T::BlockNumber) {
		let key = "contact_tracing".to_string() + "_ocw" + "::last_proccessed_block";	
		let last_processed_block_ref = StorageValueRef::persistent(key.as_bytes());
		let mut last_processed_block: u32 = match last_processed_block_ref.get::<T::BlockNumber>() {
			Some(Some(last_proccessed_block)) if last_proccessed_block >= block_number => {
				debug::info!(
					"[contact_tracing_ocw] Skipping: Block {:?} has already been processed.",
					block_number
				);
				return;
			}
			Some(Some(last_proccessed_block)) => {
				last_proccessed_block.try_into().ok().unwrap() as u32
			}
			None => 0, //TODO: define a OCW_MAX_BACKTRACK_PERIOD param
			_ => 	{
				debug::error!("[contact_tracing_ocw] Error reading contact_tracing_ocw::last_proccessed_block.");
				return;
			}
		};
		
		let start_block = last_processed_block + 1;
		let end_block = block_number.try_into().ok().unwrap() as u32;
		// debug::info!(" start_block => {} end_block => {}", start_block, end_block);
		for current_block in start_block..end_block {
			debug::info!(" start_block => {} end_block => {}", start_block, end_block);
			for e in <Self as Store>::OffchainRequests::get() {
				match e {
					OffchainRequest::FlagID(id) => {
						Self::flag_id(id);
					}
					OffchainRequest::AddToUUIDPool(id) => {
						debug::info!(" start_block => {:?}", id);
						//Self::add_uuid_pool
					}
					// there would be potential other calls
				}
			}
			debug::debug!(
				"[contact_tracing_ocw] Processing notifications for block {}",
				current_block
			);
			// let ev_indices = Self::ocw_notifications::<T::BlockNumber>(current_block.into());

			// let listener_results: Result<Vec<_>, _> = ev_indices
			// 	.iter()
			// 	.map(|idx| match Self::event_by_idx(idx) {
			// 		Some(ev) => Self::notify_listener(&ev),
			// 		None => Ok(()),
			// 	})
			// 	.collect();

			// if let Err(err) = listener_results {
			// 	debug::warn!("[contact_tracing_ocw] notify_listener error: {}", err);
			// 	break;
			// }
			last_processed_block = current_block;
		}

		if last_processed_block >= start_block {
            last_processed_block_ref.set(&last_processed_block);
            debug::info!(
                "[contact_tracing_ocw] Notifications successfully processed up to block {}",
                last_processed_block
            );
        }
	}
}

// #[derive(Default)]
// pub struct ContactBuilder<T: Trait, Moment>
// where
// {
//     id: Option<ExchangableId<T>>,
//     contact_id: Option<ExchangableId<T>>,
//     timestamp: Moment,
// }

// impl<T: Trait, Moment> ContactBuilder<T, Moment>
// where
// {
// 	pub fn add_id(mut self, id: ExchangableId<T>)-> Self {
//         self.id = id;
//         self
// 	}
	
// 	pub fn add_contact_id(mut self, contact_id: ExchangableId<T>)-> Self {
//         self.contact_id = contact_id;
//         self
//     }

// 	pub fn add_timestamp(mut self, timestamp: Moment) -> Self {
//         self.timestamp = timestamp;
//         self
//     }

// 	pub fn build(self) -> Contact<T, Moment> {
//         Contact::<T, Moment> {
//             id: self.id,
//             contact_id: self.contact_id,
//             timestamp: self.timestamp,
//         }
// 	}
	

// }
