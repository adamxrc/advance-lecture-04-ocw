#![cfg_attr(not(feature = "std"), no_std)]

/// A FRAME pallet template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references

/// For more guidance on Substrate FRAME, see the example pallet
/// https://github.com/paritytech/substrate/blob/master/frame/example/src/lib.rs

use core::{convert::TryInto};
use frame_support::{
	debug,
	decl_module,
	decl_storage,
	decl_event,
	decl_error,
	dispatch,
};
use frame_system::{
	self as system,
	ensure_signed,
	offchain::{
		Signer,
		CreateSignedTransaction,
		SendSignedTransaction,
		AppCrypto,
	},
};
use sp_core::crypto::KeyTypeId;
use sp_std::vec::Vec;
use sp_runtime::{
		offchain::{
			http,
			Duration,
		},
};
use sp_std::prelude::*;
use sp_std;
// We use `alt_serde`, and Xanewok-modified `serde_json` so that we can compile the program
//   with serde(features `std`) and alt_serde(features `no_std`).
use alt_serde::{Deserialize, Deserializer};
use codec::{Encode, Decode};

const MAX_LEN: usize = 64;

pub const DOT_REMOTE_URL: &[u8] = b"https://api.coincap.io/v2/assets/polkadot";

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default)]
struct Cryptocompare {
	#[serde(rename(deserialize = "USD"), deserialize_with = "de_float_to_integer")]
    usd: u32,
}

pub fn de_float_to_integer<'de, D>(de: D) -> Result<u32, D::Error>
where D: Deserializer<'de> {
	let f: f32 = Deserialize::deserialize(de)?;
	Ok(f as u32)
}

#[serde(crate = "alt_serde")]
#[derive(Debug, Deserialize)]
struct Coincap {
	data: AssetInfo,
}

#[serde(crate = "alt_serde")]
#[derive(Debug, Deserialize)]
struct AssetInfo {
	#[serde(rename(deserialize = "priceUsd"), deserialize_with = "de_string_to_integer")]
	price_usd: u32,
}

pub fn de_string_to_integer<'de, D>(de: D) -> Result<u32, D::Error>
where D: Deserializer<'de> {
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.parse::<f64>().unwrap() as u32)
}

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"demo");

pub mod crypto {
	use super::KEY_TYPE;
	use sp_application_crypto::{app_crypto, sr25519};

	app_crypto!(sr25519, KEY_TYPE);

	pub type AuthorityId = Public;
}

/// The pallet's configuration trait.
pub trait Trait: system::Trait + CreateSignedTransaction<Call<Self>> {
	/// The identifier type for an offchain worker.
	type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
}

// This pallet's storage items.
decl_storage! {
	// It is important to update your storage name so that your pallet's
	// storage items are isolated from other pallets.
	// ---------------------------------vvvvvvvvvvvvvv
	trait Store for Module<T: Trait> as TemplateModule {
		Prices get(fn prices): Vec<u32>;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
		NewPrice(AccountId, u32),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Value was None
		NoneValue,
		/// Value reached maximum and cannot be incremented further
		StorageOverflow,
		HttpRequestError,
		HttpBodyConvertError,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing errors
		// this includes information about your errors in the node's metadata.
		// it is needed only if you are using errors in your pallet
		type Error = Error<T>;

		// Initializing events
		// this is needed only if you are using events in your pallet
		fn deposit_event() = default;

		#[weight = 0]
		pub fn save_number(origin, price: u32) -> dispatch::DispatchResult {
			debug::info!("Entering save_number");

			let who = ensure_signed(origin)?;

			/*******
			 * 学员们在这里追加逻辑
			 *******/

			 Self::add_price(who, price);

			Ok(())
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			debug::info!("Entering off-chain workers");

			/*******
			 * 学员们在这里追加逻辑
			 *******/

			 let res = Self::fetch_price_and_signed(block_number);

			if let Err(e) = res {
				debug::error!("Submit signed: Error happends: {}", e);
			}
		}

	}
}

impl<T: Trait> Module<T> {
	fn add_price(who: T::AccountId, price: u32) {
		debug::info!("Submit signed: Adding to the prices: {}", price);
		Prices::mutate(|prices| {
			if prices.len() < MAX_LEN {
				prices.push(price);
			} else {
				prices[price as usize % MAX_LEN] = price;
			}
		});

		Self::deposit_event(RawEvent::NewPrice(who, price));
	}

	fn fetch_price_and_signed(block_number: T::BlockNumber) -> Result<(), &'static str> {
		let signer = Signer::<T, T::AuthorityId>::all_accounts();
		if !signer.can_sign() {
			return Err(
				"No local accounts available. Consider adding one via `author_insertKey` RPC."
			)?
		}

		let index: u64 = block_number.try_into().ok().unwrap() as u64;

		let dot_price = Self::fetch_price_dot().map_err(|_| "Submit signed: Failed to fetch price")?;

		debug::info!("fetch coincap price: {}  of blockNumber {}", dot_price , index);

		// Using `send_signed_transaction` associated type we create and submit a transaction
		// representing the call, we've just created.
		// Submit signed will return a vector of results for all accounts that were found in the
		// local keystore with expected `KEY_TYPE`.
		let results = signer.send_signed_transaction(
			|_account| {
				// Received price is wrapped into a call to `submit_price` public function of this pallet.
				// This means that the transaction, when executed, will simply call that function passing
				// `price` as an argument.
				Call::save_number(dot_price)
			}
		);

		for (acc, res) in &results {
			match res {
				Ok(()) => debug::info!("Submit signed: [{:?}] Submitted price of {} cents", acc.id, dot_price),
				Err(e) => debug::error!("Submit signed: [{:?}] Failed to submit transcation, {:?}", acc.id, e),
			}
		}

		Ok(())
	}

	fn fetch_price_dot() -> Result<u32, Error::<T>> {
		debug::info!("fetch coincap");
		let remote_url_bytes = DOT_REMOTE_URL.to_vec();

		let body = Self::fetch_http(&remote_url_bytes).map_err(|_| Error::<T>::HttpRequestError)?;

		let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
			debug::warn!("Not UTF8 body");
			Error::<T>::HttpBodyConvertError
		})?;

		let price_info: Coincap = serde_json::from_str(&body_str).unwrap();

		debug::warn!("Submit Signed: Got price: {} ", price_info.data.price_usd);

		Ok(price_info.data.price_usd)
	}

	fn fetch_http(remote_url_bytes: &[u8]) -> Result<Vec<u8>, http::Error> {
		let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(5000));
		// Initiate an external HTTP GET request.
		// This is using high-level wrappers from `sp_runtime`, for the low-level calls that
		// you can find in `sp_io`. The API is trying to be similar to `reqwest`, but
		// since we are running in a custom WASM execution environment we can't simply
		// import the library here.

		let remote_url_str = sp_std::str::from_utf8(&remote_url_bytes).unwrap();
		debug::info!("remote url is: {}", remote_url_str);

		let request = http::Request::get(
			remote_url_str
		);
		// We set the deadline for sending of the request, note that awaiting response can
		// have a separate deadline. Next we send the request, before that it's also possible
		// to alter request headers or stream body content in case of non-GET requests.
		let pending = request
			.deadline(deadline)
			.send()
			.map_err(|_| http::Error::IoError)?;
		// The request is already being processed by the host, we are free to do anything
		// else in the worker (we can send multiple concurrent requests too).
		// At some point however we probably want to check the response though,
		// so we can block current thread and wait for it to finish.
		// Note that since the request is being driven by the host, we don't have to wait
		// for the request to have it complete, we will just not read the response.
		let response = pending.try_wait(deadline)
			.map_err(|_| http::Error::DeadlineReached)??;

		if response.code != 200 {
			debug::warn!("Submit signed: Unexpected status code: {}", response.code);
			return Err(http::Error::Unknown);
		}

		let body = response.body().collect::<Vec<u8>>();

		Ok(body)
	}

}