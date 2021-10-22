// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

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

use crate::pallet as pallet_crowdloan;
use frame_support::parameter_types;
use frame_system::{EnsureRoot, EventRecord};
use kilt_primitives::{constants::KILT, AccountId, Balance, BlockNumber, Hash, Index};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type TestUncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type TestBlock = frame_system::mocking::MockBlock<Test>;
type TestAccountId = AccountId;
type TestBalance = Balance;
type TestOrigin = EnsureRoot<TestAccountId>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = TestBlock,
		NodeBlock = TestBlock,
		UncheckedExtrinsic = TestUncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		Crowdloan: pallet_crowdloan::{Pallet, Call, Config<T>, Storage, Event<T>}
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 38;
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = TestAccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<TestBalance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

parameter_types! {
	pub const ExistentialDeposit: TestBalance = 500;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type Balance = TestBalance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

impl pallet_crowdloan::Config for Test {
	type Currency = Balances;
	type EnsureRegistrarOrigin = TestOrigin;
	type Event = Event;
	type WeightInfo = ();
}

pub(crate) const ACCOUNT_00: TestAccountId = AccountId::new([0u8; 32]);
pub(crate) const ACCOUNT_01: TestAccountId = AccountId::new([1u8; 32]);
#[allow(clippy::identity_op)]
pub(crate) const BALANCE_01: TestBalance = 1 * KILT;
pub(crate) const BALANCE_02: TestBalance = 2 * KILT;

pub(crate) fn get_generated_events() -> Vec<EventRecord<Event, kilt_primitives::Hash>> {
	let events = System::events();
	events
		.into_iter()
		.filter(|event_details| matches!(event_details.event, Event::Crowdloan(_)))
		.collect()
}

#[derive(Default)]
pub(crate) struct ExtBuilder {
	registrar_account: TestAccountId,
	contributions: Vec<(TestAccountId, TestBalance)>,
}

impl ExtBuilder {
	pub(crate) fn with_registrar_account(mut self, account: TestAccountId) -> Self {
		self.registrar_account = account;
		self
	}

	pub(crate) fn with_contributions(mut self, contributions: Vec<(TestAccountId, TestBalance)>) -> Self {
		self.contributions = contributions;
		self
	}

	pub(crate) fn build(self) -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| {
			// Needed to test event generation.
			System::set_block_number(1);
			pallet_crowdloan::RegistrarAccount::<Test>::set(self.registrar_account);

			for (contributor_account, contribution_amount) in self.contributions.iter() {
				pallet_crowdloan::Contributions::<Test>::insert(contributor_account, contribution_amount);
			}
		});

		ext
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub(crate) fn build_with_keystore(self) -> sp_io::TestExternalities {
		use sp_keystore::{testing::KeyStore, KeystoreExt};
		use sp_std::sync::Arc;

		let mut ext = self.build();

		let keystore = KeyStore::new();
		ext.register_extension(KeystoreExt(Arc::new(keystore)));

		ext
	}
}