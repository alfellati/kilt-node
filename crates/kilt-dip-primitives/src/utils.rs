// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2024 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use pallet_dip_provider::IdentityCommitmentVersion;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::storage::StorageKey;
use sp_std::{fmt::Debug, vec::Vec};

/// The output of a type implementing the [`sp_runtime::traits::Hash`] trait.
pub type OutputOf<Hasher> = <Hasher as sp_runtime::traits::Hash>::Output;

/// The vector of vectors that implements a statically-configured maximum length
/// without requiring const generics, used in benchmarking worst cases.
#[derive(Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Debug, TypeInfo, Clone)]
pub struct BoundedBlindedValue<T>(Vec<Vec<T>>);

impl<T> BoundedBlindedValue<T> {
	pub fn into_inner(self) -> Vec<Vec<T>> {
		self.0
	}
}

impl<C, T> From<C> for BoundedBlindedValue<T>
where
	C: Iterator<Item = Vec<T>>,
{
	fn from(value: C) -> Self {
		Self(value.into_iter().collect())
	}
}

impl<T> sp_std::ops::Deref for BoundedBlindedValue<T> {
	type Target = Vec<Vec<T>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> IntoIterator for BoundedBlindedValue<T> {
	type IntoIter = <Vec<Vec<T>> as IntoIterator>::IntoIter;
	type Item = <Vec<Vec<T>> as IntoIterator>::Item;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<Context, T> kilt_support::traits::GetWorstCase<Context> for BoundedBlindedValue<T>
where
	T: Default + Clone,
{
	fn worst_case(_context: Context) -> Self {
		Self(sp_std::vec![sp_std::vec![T::default(); 128]; 64])
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T> Default for BoundedBlindedValue<T>
where
	T: Default + Clone,
{
	fn default() -> Self {
		Self(sp_std::vec![sp_std::vec![T::default(); 128]; 64])
	}
}

pub(crate) fn calculate_parachain_head_storage_key(para_id: u32) -> StorageKey {
	StorageKey(
		[
			frame_support::storage::storage_prefix(b"Paras", b"Heads").as_slice(),
			sp_io::hashing::twox_64(para_id.encode().as_ref()).as_slice(),
			para_id.encode().as_slice(),
		]
		.concat(),
	)
}

pub(crate) fn calculate_dip_identity_commitment_storage_key_for_runtime<Runtime>(
	subject: &Runtime::Identifier,
	version: IdentityCommitmentVersion,
) -> StorageKey
where
	Runtime: pallet_dip_provider::Config,
{
	StorageKey(pallet_dip_provider::IdentityCommitments::<Runtime>::hashed_key_for(
		subject, version,
	))
}
