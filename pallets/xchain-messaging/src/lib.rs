#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_module, decl_storage, decl_event, decl_error, ensure, StorageMap, Parameter, traits::BalanceStatus,
};
use frame_system::ensure_signed;
use sp_std::vec::Vec;
use sp_runtime::{DispatchResult, RuntimeDebug, traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Bounded, One, CheckedAdd, Zero}};
use orml_traits::{MultiReservableCurrency, MultiCurrency};
use orml_utilities::with_transaction_result;
use codec::{Encode, Decode};

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Config: frame_system::Config {
    /// Because this pallet emits events, it depends on the runtime's definition of an event.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type MessageId: Parameter + AtLeast32BitUnsigned + Default + Copy + MaybeSerializeDeserialize + Bounded;
    type Currency: MultiReservableCurrency<Self::AccountId>;
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Eq, PartialEq)]
pub struct Message<CurrencyId, Balance, AccountId> {
	pub base_currency_id: CurrencyId,
	#[codec(compact)]
	pub base_amount: Balance,
	pub target_currency_id: CurrencyId,
	#[codec(compact)]
	pub target_amount: Balance,
	pub owner: AccountId,
}


type BalanceOf<T> = <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
type MessageOf<T> = Message<CurrencyIdOf<T>, BalanceOf<T>, <T as frame_system::Config>::AccountId>;

decl_event! {

    pub enum Event<T> where
		<T as Config>::MessageId,
		Message = MessageOf<T>,
		<T as frame_system::Config>::AccountId,
	{
		MessageSubmited(MessageId, Message),
		MessageProcessed(AccountId, MessageId, Message),
		MessageCancelled(MessageId),
	}
}

// Errors inform users that something went wrong.
decl_error! {
    pub enum Error for Module<T: Config> {
        MessageIdInvalid,
        MessageIdOverflow,
        NotMessageOwner,
        InsufficientBalance,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as TemplateModule {
        Messages: map hasher(blake2_128_concat) T::MessageId => Option<MessageOf<T>>;
        pub NextMessageId: T::MessageId;
    }
}



decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = 10_000]
        fn create_claim(
            origin,
			base_currency_id: CurrencyIdOf<T>,
			base_amount: BalanceOf<T>,
			target_currency_id: CurrencyIdOf<T>,
			target_amount: BalanceOf<T>,
        ) {
            let who = ensure_signed(origin)?;

			NextMessageId::<T>::try_mutate(|id| -> DispatchResult {
				let message_id = *id;

				let message = Message {
					base_currency_id,
					base_amount,
					target_currency_id,
					target_amount,
					owner: who.clone(),
				};

				*id = id.checked_add(&One::one()).ok_or(Error::<T>::MessageIdOverflow)?;
				
				T::Currency::reserve(base_currency_id, &who, base_amount)?;

				Messages::<T>::insert(message_id, &message);

				Self::deposit_event(RawEvent::MessageSubmited(message_id, message));
				Ok(())
			})?;
        }
        
        #[weight = 10_000]
        fn process_message(origin, message_id: T::MessageId) {
            let who = ensure_signed(origin)?;

			Messages::<T>::try_mutate_exists(message_id, |message| -> DispatchResult {
				let message = message.take().ok_or(Error::<T>::MessageIdInvalid)?;

				with_transaction_result(|| {
					T::Currency::transfer(message.target_currency_id, &who, &message.owner, message.target_amount)?;
					let val = T::Currency::repatriate_reserved(message.base_currency_id, &message.owner, &who, message.base_amount, BalanceStatus::Free)?;
					ensure!(val.is_zero(), Error::<T>::InsufficientBalance);

					Self::deposit_event(RawEvent::MessageProcessed(who, message_id, message));

					Ok(())
				})
			})?;
        }

        #[weight = 10_000]
        fn cancel_message(origin, message_id: T::MessageId) {
            let who = ensure_signed(origin)?;

			Messages::<T>::try_mutate_exists(message_id, |message| -> DispatchResult {
                let message = message.take().ok_or(Error::<T>::MessageIdInvalid)?;

                ensure!(message.owner == who, Error::<T>::NotMessageOwner);

                Self::deposit_event(RawEvent::MessageCancelled(message_id));

                Ok(())
            })?;
        }
    }
}

impl<T: Config> Module<T> {}