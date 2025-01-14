// Ajuna Node
// Copyright (C) 2022 BlogaTech AG

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.

// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![feature(map_first_last)]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

pub mod types;

#[frame_support::pallet]
pub mod pallet {
	use super::types::*;
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ExistenceRequirement, Randomness, WithdrawReasons},
	};
	use frame_system::{ensure_root, ensure_signed, pallet_prelude::OriginFor};
	use sp_runtime::{
		traits::{Hash, Saturating, TrailingZeroInput, UniqueSaturatedInto},
		ArithmeticError,
	};
	use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

	pub(crate) type SeasonOf<T> = Season<<T as frame_system::Config>::BlockNumber>;
	pub(crate) type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub(crate) type AvatarIdOf<T> = <T as frame_system::Config>::Hash;
	pub(crate) type BoundedAvatarIdsOf<T> = BoundedVec<AvatarIdOf<T>, ConstU32<4_294_967_295>>;
	pub(crate) type GlobalConfigOf<T> =
		GlobalConfig<BalanceOf<T>, <T as frame_system::Config>::BlockNumber>;

	pub(crate) const MAX_PERCENTAGE: u8 = 100;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::storage]
	#[pallet::getter(fn organizer)]
	pub type Organizer<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::type_value]
	pub fn DefaultSeasonId() -> SeasonId {
		1
	}

	#[pallet::storage]
	#[pallet::getter(fn current_season_id)]
	pub type CurrentSeasonId<T: Config> = StorageValue<_, SeasonId, ValueQuery, DefaultSeasonId>;

	#[pallet::storage]
	#[pallet::getter(fn is_season_active)]
	pub type IsSeasonActive<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// Storage for the seasons.
	#[pallet::storage]
	#[pallet::getter(fn seasons)]
	pub type Seasons<T: Config> = StorageMap<_, Identity, SeasonId, SeasonOf<T>, OptionQuery>;

	#[pallet::type_value]
	pub fn DefaultGlobalConfig<T: Config>() -> GlobalConfigOf<T> {
		GlobalConfig {
			max_avatars_per_player: 1_000,
			mint: MintConfig {
				open: true,
				fees: MintFees {
					one: (1_000_000_000_000_u64 * 55 / 100).unique_saturated_into(),
					three: (1_000_000_000_000_u64 * 50 / 100).unique_saturated_into(),
					six: (1_000_000_000_000_u64 * 45 / 100).unique_saturated_into(),
				},
				cooldown: 5_u8.into(),
				free_mint_transfer_fee: 1,
			},
			forge: ForgeConfig { open: true, min_sacrifices: 1, max_sacrifices: 4 },
			trade: TradeConfig { open: true },
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn global_configs)]
	pub type GlobalConfigs<T: Config> =
		StorageValue<_, GlobalConfigOf<T>, ValueQuery, DefaultGlobalConfig<T>>;

	#[pallet::storage]
	#[pallet::getter(fn avatars)]
	pub type Avatars<T: Config> = StorageMap<_, Identity, AvatarIdOf<T>, (T::AccountId, Avatar)>;

	#[pallet::storage]
	#[pallet::getter(fn owners)]
	pub type Owners<T: Config> =
		StorageMap<_, Identity, T::AccountId, BoundedAvatarIdsOf<T>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_minted_block_numbers)]
	pub type LastMintedBlockNumbers<T: Config> =
		StorageMap<_, Identity, T::AccountId, T::BlockNumber, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn free_mints)]
	pub type FreeMints<T: Config> = StorageMap<_, Identity, T::AccountId, MintCount, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn trade)]
	pub type Trade<T: Config> = StorageMap<_, Identity, AvatarIdOf<T>, BalanceOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new organizer has been set.
		OrganizerSet { organizer: T::AccountId },
		/// The season configuration for {season_id} has been updated.
		UpdatedSeason { season_id: SeasonId, season: SeasonOf<T> },
		/// Global configuration updated.
		UpdatedGlobalConfig(GlobalConfigOf<T>),
		/// Avatars minted.
		AvatarsMinted { avatar_ids: Vec<AvatarIdOf<T>> },
		/// Avatar forged.
		AvatarForged { avatar_id: AvatarIdOf<T>, upgraded_components: u8 },
		/// A season has started.
		SeasonStarted(SeasonId),
		/// A season has finished.
		SeasonFinished(SeasonId),
		/// Free mints transfered between accounts.
		FreeMintsTransferred { from: T::AccountId, to: T::AccountId, how_many: MintCount },
		/// Free mints issued to account.
		FreeMintsIssued { to: T::AccountId, how_many: MintCount },
		/// Avatar has price set for trade.
		AvatarPriceSet { avatar_id: AvatarIdOf<T>, price: BalanceOf<T> },
		/// Avatar has price removed for trade.
		AvatarPriceUnset { avatar_id: AvatarIdOf<T> },
		/// Avatar has been traded.
		AvatarTraded { avatar_id: AvatarIdOf<T>, from: T::AccountId, to: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no account set as the organizer
		OrganizerNotSet,
		/// The season starts before the previous season has ended.
		EarlyStartTooEarly,
		/// The season season start later than its early access
		EarlyStartTooLate,
		/// The season start date is newer than its end date.
		SeasonStartTooLate,
		/// The season ends after the new season has started.
		SeasonEndTooLate,
		/// The season doesn't exist.
		UnknownSeason,
		/// The avatar doesn't exist.
		UnknownAvatar,
		/// The avatar for sale doesn't exist.
		UnknownAvatarForSale,
		/// The tier doesn't exist.
		UnknownTier,
		/// The season ID of a season to create is not sequential.
		NonSequentialSeasonId,
		/// Rarity percentages don't add up to 100
		IncorrectRarityPercentages,
		/// Max tier is achievable through forging only. Therefore the number of rarity percentages
		/// must be less than that of tiers for a season.
		TooManyRarityPercentages,
		/// Some rarity tier are duplicated.
		DuplicatedRarityTier,
		/// Minting is not available at the moment.
		MintClosed,
		/// Forging is not available at the moment.
		ForgeClosed,
		/// Trading is not available at the moment.
		TradeClosed,
		/// No season active currently.
		OutOfSeason,
		/// Max ownership reached.
		MaxOwnershipReached,
		/// Avatar belongs to someone else.
		Ownership,
		/// Incorrect DNA.
		IncorrectDna,
		/// Incorrect Avatar ID.
		IncorrectAvatarId,
		/// The player must wait cooldown period.
		MintCooldown,
		/// The player has not enough funds.
		InsufficientFunds,
		/// The season's max components value is less than the minimum allowed (1).
		MaxComponentsTooLow,
		/// The season's max components value is more than the maximum allowed (random byte: 32).
		MaxComponentsTooHigh,
		/// The season's max variations value is less than the minimum allowed (1).
		MaxVariationsTooLow,
		/// The season's max variations value is more than the maximum allowed (15).
		MaxVariationsTooHigh,
		/// The player has not enough free mints available.
		InsufficientFreeMints,
		/// Less than minimum allowed sacrifices are used for forging.
		TooFewSacrifices,
		/// More than maximum allowed sacrifices are used for forging.
		TooManySacrifices,
		/// Leader is being sacrificed.
		LeaderSacrificed,
		/// An avatar listed for trade is used to forge.
		AvatarInTrade,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn mint(origin: OriginFor<T>, mint_option: MintOption) -> DispatchResult {
			let player = ensure_signed(origin)?;
			Self::toggle_season();
			Self::do_mint(&player, &mint_option)
		}

		#[pallet::weight(10_000)]
		pub fn forge(
			origin: OriginFor<T>,
			leader: AvatarIdOf<T>,
			sacrifices: BoundedAvatarIdsOf<T>,
		) -> DispatchResult {
			let player = ensure_signed(origin)?;
			Self::toggle_season();
			Self::do_forge(&player, &leader, &sacrifices)
		}

		#[pallet::weight(10_000)]
		pub fn transfer_free_mints(
			origin: OriginFor<T>,
			dest: T::AccountId,
			how_many: MintCount,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let GlobalConfig { mint, .. } = Self::global_configs();
			let sender_free_mints = FreeMints::<T>::get(&sender)
				.checked_sub(
					how_many
						.checked_add(mint.free_mint_transfer_fee)
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(Error::<T>::InsufficientFreeMints)?;
			let dest_free_mints = FreeMints::<T>::get(&dest)
				.checked_add(how_many)
				.ok_or(ArithmeticError::Overflow)?;

			FreeMints::<T>::insert(&sender, sender_free_mints);
			FreeMints::<T>::insert(&dest, dest_free_mints);

			Self::deposit_event(Event::FreeMintsTransferred { from: sender, to: dest, how_many });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn set_price(
			origin: OriginFor<T>,
			avatar_id: AvatarIdOf<T>,
			#[pallet::compact] price: BalanceOf<T>,
		) -> DispatchResult {
			let seller = ensure_signed(origin)?;
			ensure!(Self::global_configs().trade.open, Error::<T>::TradeClosed);
			let _ = Self::ensure_ownership(&seller, &avatar_id)?;
			Trade::<T>::insert(&avatar_id, &price);
			Self::deposit_event(Event::AvatarPriceSet { avatar_id, price });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn remove_price(origin: OriginFor<T>, avatar_id: AvatarIdOf<T>) -> DispatchResult {
			let seller = ensure_signed(origin)?;
			ensure!(Self::global_configs().trade.open, Error::<T>::TradeClosed);
			Self::ensure_for_trade(&avatar_id)?;
			Self::ensure_ownership(&seller, &avatar_id)?;
			Trade::<T>::remove(&avatar_id);
			Self::deposit_event(Event::AvatarPriceUnset { avatar_id });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn buy(origin: OriginFor<T>, avatar_id: AvatarIdOf<T>) -> DispatchResult {
			let buyer = ensure_signed(origin)?;
			ensure!(Self::global_configs().trade.open, Error::<T>::TradeClosed);
			let (seller, price) = Self::ensure_for_trade(&avatar_id)?;
			ensure!(T::Currency::free_balance(&buyer) >= price, Error::<T>::InsufficientFunds);

			T::Currency::transfer(&buyer, &seller, price, ExistenceRequirement::KeepAlive)?;

			let mut buyer_avatar_ids = Self::owners(&buyer);
			buyer_avatar_ids
				.try_push(avatar_id)
				.map_err(|_| Error::<T>::IncorrectAvatarId)?;

			let mut seller_avatar_ids = Self::owners(&seller);
			seller_avatar_ids.retain(|x| x != &avatar_id);

			Owners::<T>::mutate(&buyer, |avatar_ids| *avatar_ids = buyer_avatar_ids);
			Owners::<T>::mutate(&seller, |avatar_ids| *avatar_ids = seller_avatar_ids);
			Avatars::<T>::mutate(&avatar_id, |maybe_avatar| {
				if let Some((owner, _)) = maybe_avatar {
					*owner = buyer.clone();
				}
			});
			Trade::<T>::remove(&avatar_id);

			Self::deposit_event(Event::AvatarTraded { avatar_id, from: seller, to: buyer });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn set_organizer(origin: OriginFor<T>, organizer: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			Organizer::<T>::put(&organizer);
			Self::deposit_event(Event::OrganizerSet { organizer });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn upsert_season(
			origin: OriginFor<T>,
			season_id: SeasonId,
			season: SeasonOf<T>,
		) -> DispatchResult {
			Self::ensure_organizer(origin)?;
			let season = Self::ensure_season(&season_id, season)?;
			Seasons::<T>::insert(season_id, &season);
			Self::deposit_event(Event::UpdatedSeason { season_id, season });
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn update_global_config(
			origin: OriginFor<T>,
			new_global_config: GlobalConfigOf<T>,
		) -> DispatchResult {
			Self::ensure_organizer(origin)?;
			GlobalConfigs::<T>::put(&new_global_config);
			Self::deposit_event(Event::UpdatedGlobalConfig(new_global_config));
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn issue_free_mints(
			origin: OriginFor<T>,
			dest: T::AccountId,
			how_many: MintCount,
		) -> DispatchResult {
			Self::ensure_organizer(origin)?;
			let dest_free_mints = FreeMints::<T>::get(&dest)
				.checked_add(how_many)
				.ok_or(ArithmeticError::Overflow)?;
			FreeMints::<T>::insert(&dest, dest_free_mints);
			Self::deposit_event(Event::FreeMintsIssued { to: dest, how_many });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn ensure_organizer(origin: OriginFor<T>) -> DispatchResult {
			let maybe_organizer = ensure_signed(origin)?;
			let existing_organizer = Organizer::<T>::get().ok_or(Error::<T>::OrganizerNotSet)?;
			ensure!(maybe_organizer == existing_organizer, DispatchError::BadOrigin);
			Ok(())
		}

		pub(crate) fn ensure_season(
			season_id: &SeasonId,
			mut season: SeasonOf<T>,
		) -> Result<SeasonOf<T>, DispatchError> {
			season.validate::<T>()?;

			let prev_season_id = season_id.checked_sub(1).ok_or(ArithmeticError::Underflow)?;
			let next_season_id = season_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			if prev_season_id > 0 {
				let prev_season =
					Self::seasons(prev_season_id).ok_or(Error::<T>::NonSequentialSeasonId)?;
				ensure!(prev_season.end < season.early_start, Error::<T>::EarlyStartTooEarly);
			}
			if let Some(next_season) = Self::seasons(next_season_id) {
				ensure!(season.end < next_season.early_start, Error::<T>::SeasonEndTooLate);
			}
			Ok(season)
		}

		#[inline]
		fn random_hash(phrase: &[u8], who: &T::AccountId) -> T::Hash {
			let (seed, _) = T::Randomness::random(phrase);
			let seed = T::Hash::decode(&mut TrailingZeroInput::new(seed.as_ref()))
				.expect("input is padded with zeroes; qed");
			let nonce = frame_system::Pallet::<T>::account_nonce(who);
			frame_system::Pallet::<T>::inc_account_nonce(who);
			(seed, &who, nonce.encode()).using_encoded(T::Hashing::hash)
		}

		#[inline]
		fn random_component(
			season: &SeasonOf<T>,
			hash: &T::Hash,
			index: usize,
			batched_mint: bool,
		) -> (u8, u8) {
			let hash = hash.as_ref();
			let random_tier = {
				let random_p = (hash[index] % MAX_PERCENTAGE) as u16;
				let p = if batched_mint { &season.p_batch_mint } else { &season.p_single_mint };
				let mut cumulative_sum = 0;
				let mut random_tier = season.tiers[0].clone() as u8;
				for i in 0..p.len() {
					let new_cumulative_sum = cumulative_sum + p[i];
					if random_p >= cumulative_sum && random_p < new_cumulative_sum {
						random_tier = season.tiers[i].clone() as u8;
						break
					}
					cumulative_sum = new_cumulative_sum;
				}
				random_tier
			};
			let random_variation = hash[index + 1] % season.max_variations;
			(random_tier, random_variation)
		}

		fn random_dna(
			hash: &T::Hash,
			season: &SeasonOf<T>,
			batched_mint: bool,
		) -> Result<Dna, DispatchError> {
			let dna = (0..season.max_components)
				.map(|i| {
					let (random_tier, random_variation) =
						Self::random_component(season, hash, i as usize * 2, batched_mint);
					((random_tier << 4) | random_variation) as u8
				})
				.collect::<Vec<_>>();
			Dna::try_from(dna).map_err(|_| Error::<T>::IncorrectDna.into())
		}

		pub(crate) fn do_mint(player: &T::AccountId, mint_option: &MintOption) -> DispatchResult {
			let GlobalConfig { max_avatars_per_player, mint, .. } = Self::global_configs();
			ensure!(mint.open, Error::<T>::MintClosed);

			let current_block = <frame_system::Pallet<T>>::block_number();
			if let Some(last_block) = Self::last_minted_block_numbers(&player) {
				ensure!(current_block > last_block + mint.cooldown, Error::<T>::MintCooldown);
			}

			let MintOption { mint_type, count } = mint_option;
			match mint_type {
				MintType::Normal => {
					let fee = mint.fees.fee_for(*count);
					ensure!(
						T::Currency::free_balance(player) >= fee,
						Error::<T>::InsufficientFunds
					);
				},
				MintType::Free => ensure!(
					Self::free_mints(player) >= *count as MintCount,
					Error::<T>::InsufficientFreeMints
				),
			};

			let how_many = *count as usize;
			let max_ownership = (max_avatars_per_player as usize)
				.checked_sub(how_many)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(Self::owners(player).len() <= max_ownership, Error::<T>::MaxOwnershipReached);

			ensure!(Self::is_season_active(), Error::<T>::OutOfSeason);
			let season_id = Self::current_season_id();
			let season = Self::seasons(season_id).ok_or(Error::<T>::UnknownSeason)?;

			let generated_avatars = (0..how_many)
				.map(|_| {
					let avatar_id = Self::random_hash(b"create_avatar", player);
					let dna = Self::random_dna(&avatar_id, &season, how_many > 1)?;
					let souls = (dna.iter().map(|x| *x as SoulCount).sum::<SoulCount>() % 100) + 1;
					let avatar = Avatar { season_id, dna, souls };
					Ok((avatar_id, avatar))
				})
				.collect::<Result<Vec<(AvatarIdOf<T>, Avatar)>, DispatchError>>()?;

			Owners::<T>::try_mutate(&player, |avatar_ids| -> DispatchResult {
				let generated_avatars_ids = generated_avatars
					.into_iter()
					.map(|(avatar_id, avatar)| {
						Avatars::<T>::insert(avatar_id, (&player, avatar));
						avatar_id
					})
					.collect::<Vec<_>>();
				ensure!(
					avatar_ids.try_extend(generated_avatars_ids.clone().into_iter()).is_ok(),
					Error::<T>::MaxOwnershipReached
				);
				LastMintedBlockNumbers::<T>::insert(&player, current_block);

				match mint_type {
					MintType::Normal => {
						T::Currency::withdraw(
							player,
							mint.fees.fee_for(*count),
							WithdrawReasons::FEE,
							ExistenceRequirement::KeepAlive,
						)?;
					},
					MintType::Free => {
						let _ = FreeMints::<T>::try_mutate(
							player,
							|dest_player_free_mints| -> DispatchResult {
								*dest_player_free_mints = dest_player_free_mints
									.checked_sub(*count as MintCount)
									.ok_or(ArithmeticError::Overflow)?;
								Ok(())
							},
						);
					},
				};

				Self::deposit_event(Event::AvatarsMinted { avatar_ids: generated_avatars_ids });
				Ok(())
			})
		}

		pub(crate) fn do_forge(
			player: &T::AccountId,
			leader: &AvatarIdOf<T>,
			sacrifices: &BoundedAvatarIdsOf<T>,
		) -> DispatchResult {
			let GlobalConfig { forge, .. } = Self::global_configs();
			ensure!(sacrifices.len() as u8 >= forge.min_sacrifices, Error::<T>::TooFewSacrifices);
			ensure!(sacrifices.len() as u8 <= forge.max_sacrifices, Error::<T>::TooManySacrifices);
			ensure!(forge.open, Error::<T>::ForgeClosed);

			ensure!(Self::is_season_active(), Error::<T>::OutOfSeason);
			let Season { max_variations, tiers, .. } =
				Self::seasons(Self::current_season_id()).ok_or(Error::<T>::UnknownSeason)?;
			let max_tier = tiers.iter().max().ok_or(Error::<T>::UnknownTier)?.clone() as u8;

			ensure!(Self::ensure_for_trade(leader).is_err(), Error::<T>::AvatarInTrade);
			let mut leader_avatar = Self::ensure_ownership(player, leader)?;
			type Accumulator<T> = (BTreeSet<usize>, Vec<AvatarIdOf<T>>, u8);
			let (mut unique_matched_indexes, _, matches) = sacrifices
				.iter()
				.try_fold::<Accumulator<T>, _, Result<Accumulator<T>, DispatchError>>(
					Accumulator::<T>::default(),
					|(mut matched_components, mut seen, mut matches), sacrifice| {
						ensure!(leader != sacrifice, Error::<T>::LeaderSacrificed);
						if !seen.contains(sacrifice) {
							ensure!(
								Self::ensure_for_trade(sacrifice).is_err(),
								Error::<T>::AvatarInTrade
							);
							let sacrifice_avatar = Self::ensure_ownership(player, sacrifice)?;
							let (is_match, matching_components) =
								leader_avatar.compare(&sacrifice_avatar, max_variations, max_tier);

							if is_match {
								matches += 1;
								matched_components.extend(matching_components.iter());
							}

							leader_avatar.souls = leader_avatar
								.souls
								.checked_add(sacrifice_avatar.souls)
								.ok_or(ArithmeticError::Overflow)?;
							seen.push(*sacrifice);
						}
						Ok((matched_components, seen, matches))
					},
				)?;

			let random_hash = Self::random_hash(b"forging avatar", player);
			let random_hash = random_hash.as_ref();
			let mut upgraded_components = 0;

			// all matches approx. 100%
			let p = (MAX_PERCENTAGE / forge.max_sacrifices) * matches;
			let rolls = sacrifices.len();
			for hash in random_hash.iter().take(rolls) {
				if let Some(first_matched_index) = unique_matched_indexes.pop_first() {
					let roll = hash % MAX_PERCENTAGE;
					if roll <= p {
						let nucleotide = leader_avatar.dna[first_matched_index];
						let current_tier_index = tiers
							.iter()
							.position(|tier| tier.clone() as u8 == nucleotide >> 4)
							.ok_or(Error::<T>::UnknownTier)?;

						let already_maxed_out = current_tier_index == (tiers.len() - 1);
						if !already_maxed_out {
							let next_tier = tiers[current_tier_index + 1].clone() as u8;
							let upgraded_nucleotide = (next_tier << 4) | (nucleotide & 0b0000_1111);
							leader_avatar.dna[first_matched_index] = upgraded_nucleotide;
							upgraded_components += 1;
						}
					}
				}
			}

			Avatars::<T>::insert(leader, (player, leader_avatar));
			sacrifices.iter().for_each(Avatars::<T>::remove);
			let remaining_avatar_ids = Owners::<T>::take(player)
				.into_iter()
				.filter(|avatar_id| !sacrifices.contains(avatar_id))
				.collect::<Vec<_>>();
			let remaining_avatar_ids = BoundedAvatarIdsOf::<T>::try_from(remaining_avatar_ids)
				.map_err(|_| Error::<T>::IncorrectAvatarId)?;
			Owners::<T>::insert(player, remaining_avatar_ids);

			Self::deposit_event(Event::AvatarForged { avatar_id: *leader, upgraded_components });
			Ok(())
		}

		fn ensure_ownership(
			player: &T::AccountId,
			avatar_id: &AvatarIdOf<T>,
		) -> Result<Avatar, DispatchError> {
			let (owner, avatar) = Self::avatars(avatar_id).ok_or(Error::<T>::UnknownAvatar)?;
			ensure!(player == &owner, Error::<T>::Ownership);
			Ok(avatar)
		}

		fn ensure_for_trade(
			avatar_id: &AvatarIdOf<T>,
		) -> Result<(T::AccountId, BalanceOf<T>), DispatchError> {
			let price = Self::trade(avatar_id).ok_or(Error::<T>::UnknownAvatarForSale)?;
			let (seller, _) = Self::avatars(avatar_id).ok_or(Error::<T>::UnknownAvatar)?;
			Ok((seller, price))
		}

		fn toggle_season() {
			let mut current_season_id = Self::current_season_id();
			if let Some(season) = Self::seasons(&current_season_id) {
				let now = <frame_system::Pallet<T>>::block_number();
				let is_active = now >= season.early_start && now <= season.end;
				let is_currently_active = Self::is_season_active();

				// activate season
				if !is_currently_active && is_active {
					Self::activate_season(current_season_id);
				}

				// deactivate season (and active if condition met)
				if now > season.end {
					Self::deactivate_season();
					current_season_id.saturating_inc();
					if let Some(next_season) = Self::seasons(current_season_id) {
						if now >= next_season.early_start {
							Self::activate_season(current_season_id);
						}
					}
				}
			}
		}

		fn activate_season(season_id: SeasonId) {
			IsSeasonActive::<T>::put(true);
			Self::deposit_event(Event::SeasonStarted(season_id));
		}

		fn deactivate_season() {
			IsSeasonActive::<T>::put(false);
			CurrentSeasonId::<T>::mutate(|season_id| {
				Self::deposit_event(Event::SeasonFinished(*season_id));
				season_id.saturating_inc()
			});
		}
	}
}
