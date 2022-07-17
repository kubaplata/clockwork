use {
    super::{Node, Snapshot},
    crate::{
        errors::CronosError,
        state::{NodeAccount, SnapshotAccount, SnapshotStatus},
    },
    anchor_lang::{prelude::*, AnchorDeserialize},
    anchor_spl::token::TokenAccount,
    std::convert::TryFrom,
};

pub const SEED_REGISTRY: &[u8] = b"registry";

/**
 * Registry
 */

#[account]
#[derive(Debug)]
pub struct Registry {
    pub is_locked: bool,
    pub node_count: u64,
    pub snapshot_count: u64,
}

impl Registry {
    pub fn pubkey() -> Pubkey {
        Pubkey::find_program_address(&[SEED_REGISTRY], &crate::ID).0
    }
}

impl TryFrom<Vec<u8>> for Registry {
    type Error = Error;
    fn try_from(data: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        Registry::try_deserialize(&mut data.as_slice())
    }
}

/**
 * RegistryAccount
 */

pub trait RegistryAccount {
    fn new(&mut self) -> Result<()>;

    fn new_node(
        &mut self,
        owner: &mut Signer,
        node: &mut Account<Node>,
        stake: &mut Account<TokenAccount>,
        worker: &Signer,
    ) -> Result<()>;

    fn new_snapshot(&mut self, snapshot: &mut Account<Snapshot>) -> Result<()>;

    fn rotate_snapshot(
        &mut self,
        current_snapshot: Option<&mut Account<Snapshot>>,
        next_snapshot: &mut Account<Snapshot>,
    ) -> Result<()>;

    fn lock(&mut self) -> Result<()>;

    fn unlock(&mut self) -> Result<()>;
}

impl RegistryAccount for Account<'_, Registry> {
    fn new(&mut self) -> Result<()> {
        self.is_locked = false;
        self.node_count = 0;
        self.snapshot_count = 0;
        Ok(())
    }

    fn new_node(
        &mut self,
        owner: &mut Signer,
        node: &mut Account<Node>,
        stake: &mut Account<TokenAccount>,
        worker: &Signer,
    ) -> Result<()> {
        require!(!self.is_locked, CronosError::RegistryLocked);
        node.new(self.node_count, owner, stake, worker)?;
        self.node_count = self.node_count.checked_add(1).unwrap();
        Ok(())
    }

    fn new_snapshot(&mut self, snapshot: &mut Account<Snapshot>) -> Result<()> {
        require!(!self.is_locked, CronosError::RegistryLocked);
        self.lock()?;
        snapshot.new(self.snapshot_count)?;
        Ok(())
    }

    fn rotate_snapshot(
        &mut self,
        current_snapshot: Option<&mut Account<Snapshot>>,
        next_snapshot: &mut Account<Snapshot>,
    ) -> Result<()> {
        // Require the registry is locked
        require!(self.is_locked, CronosError::RegistryMustBeLocked);

        // Validate the next snapshot is in progress
        require!(
            next_snapshot.status == SnapshotStatus::InProgress,
            CronosError::SnapshotNotInProgress
        );

        // Validate the snapshot has captured the entire registry
        require!(
            next_snapshot.node_count == self.node_count,
            CronosError::SnapshotIncomplete
        );

        // Get current timestamp
        let ts = Clock::get().unwrap().unix_timestamp;

        // Archive the current snapshot
        match current_snapshot {
            Some(current_snapshot) => {
                // Validate the snapshot is current
                require!(
                    current_snapshot.status == SnapshotStatus::Current,
                    CronosError::SnapshotNotCurrent
                );

                // Mark the current snapshot as archived
                current_snapshot.status = SnapshotStatus::Archived { ts };
            }
            None => require!(self.snapshot_count == 0, CronosError::SnapshotNotCurrent),
        }

        // Mark the next snapshot as current
        next_snapshot.status = SnapshotStatus::Current;

        // Increment snapshot counter
        self.snapshot_count = self.snapshot_count.checked_add(1).unwrap();

        // Unlock the registry
        self.unlock()?;

        Ok(())
    }

    fn lock(&mut self) -> Result<()> {
        self.is_locked = true;
        Ok(())
    }

    fn unlock(&mut self) -> Result<()> {
        self.is_locked = false;
        Ok(())
    }
}
