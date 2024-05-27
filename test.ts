import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Port3LayerTwoStaking } from "../target/types/port3_layer2_staking";
import { assert } from "chai";

describe("port3-layer2-staking", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Port3LayerTwoStaking as Program<Port3LayerTwoStaking>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize(program.provider.wallet.publicKey)
      .accounts({
        owner: program.provider.wallet.publicKey,
        port3Vault: "your_port3_vault_account_pubkey",
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Your transaction signature", tx);

    // Verify the initialized state
    const poolInfo = await program.account.poolInfo.fetch(
      program.provider.wallet.publicKey
    );
    assert.equal(poolInfo.owner.toString(), program.provider.wallet.publicKey.toString());
    assert.isFalse(poolInfo.isPaused);
    assert.equal(poolInfo.totalMintReward.toNumber(), 0);
    assert.equal(poolInfo.totalEthMintReward.toNumber(), 0);
    assert.equal(poolInfo.feePerThousand.toNumber(), 0);
    assert.equal(poolInfo.totalStaking.toNumber(), 0);
    assert.equal(poolInfo.rewardThreshold.toNumber(), 50000 * 10 ** 9);
  });

  it("Deposits tokens", async () => {
    // Prepare test accounts
    const [user, lpToken, lpTokenAccount] = await createTestAccounts(program);

    // Call the deposit function
    const tx = await program.methods.deposit(0, 1000)
      .accounts({
        user: user.publicKey,
        userInfo: user.publicKey,
        poolInfo: program.provider.wallet.publicKey,
        lpTokenAccount: lpTokenAccount.publicKey,
        feeAccount: lpTokenAccount.publicKey,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    console.log("Deposit transaction signature", tx);

    // Verify the updated state
    const userInfo = await program.account.userInfo.fetch(user.publicKey);
    assert.equal(userInfo.amount.toNumber(), 1000);

    const poolInfo = await program.account.poolInfo.fetch(program.provider.wallet.publicKey);
    assert.equal(poolInfo.amount.toNumber(), 1000);
  });

  // Add more test cases for other functions like withdraw, emergency withdraw, etc.
});

async function createTestAccounts(
  program: Program<Port3LayerTwoStaking>
): Promise<[anchor.web3.Keypair, anchor.web3.Keypair, anchor.web3.PublicKey]> {
  // Create a new user keypair
  const user = anchor.web3.Keypair.generate();

  // Create a new LP token mint and account
  const lpToken = anchor.web3.Keypair.generate();
  const lpTokenAccount = await createTokenAccount(program, lpToken.publicKey, user.publicKey);

  return [user, lpToken, lpTokenAccount];
}

async function createTokenAccount(
  program: Program<Port3LayerTwoStaking>,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey
): Promise<anchor.web3.PublicKey> {
  const [tokenAccount] = await anchor.web3.PublicKey.findProgramAddress(
    [owner.toBuffer(), anchor.utils.token.TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    anchor.utils.token.TOKEN_PROGRAM_ID
  );

  await program.methods
    .initializeTokenAccount(mint)
    .accounts({
      tokenAccount,
      owner,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    })
    .rpc();

  return tokenAccount;
}

it("Withdraws tokens", async () => {
  // Prepare test accounts
  const [user, lpToken, lpTokenAccount] = await createTestAccounts(program);

  // Deposit some tokens first
  await program.methods.deposit(0, 1000)
    .accounts({
      user: user.publicKey,
      userInfo: user.publicKey,
      poolInfo: program.provider.wallet.publicKey,
      lpTokenAccount: lpTokenAccount.publicKey,
      feeAccount: lpTokenAccount.publicKey,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    })
    .signers([user])
    .rpc();

  // Call the withdraw function
  const tx = await program.methods.withdraw(0, 500)
    .accounts({
      user: user.publicKey,
      userInfo: user.publicKey,
      poolInfo: program.provider.wallet.publicKey,
      lpTokenAccount: lpTokenAccount.publicKey,
      destination: lpTokenAccount.publicKey,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    })
    .signers([user])
    .rpc();

  console.log("Withdraw transaction signature", tx);

  // Verify the updated state
  const userInfo = await program.account.userInfo.fetch(user.publicKey);
  assert.equal(userInfo.amount.toNumber(), 500);

  const poolInfo = await program.account.poolInfo.fetch(program.provider.wallet.publicKey);
  assert.equal(poolInfo.amount.toNumber(), 500);
});

it("Performs emergency withdraw", async () => {
  // Prepare test accounts
  const [user, lpToken, lpTokenAccount] = await createTestAccounts(program);

  // Deposit some tokens first
  await program.methods.deposit(0, 1000)
    .accounts({
      user: user.publicKey,
      userInfo: user.publicKey,
      poolInfo: program.provider.wallet.publicKey,
      lpTokenAccount: lpTokenAccount.publicKey,
      feeAccount: lpTokenAccount.publicKey,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    })
    .signers([user])
    .rpc();

  // Call the emergency withdraw function
  const tx = await program.methods.emergencyWithdraw(0)
    .accounts({
      user: user.publicKey,
      userInfo: user.publicKey,
      poolInfo: program.provider.wallet.publicKey,
      lpTokenAccount: lpTokenAccount.publicKey,
      destination: lpTokenAccount.publicKey,
      tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
    })
    .signers([user])
    .rpc();

  console.log("Emergency withdraw transaction signature", tx);

  // Verify the updated state
  const userInfo = await program.account.userInfo.fetch(user.publicKey);
  assert.equal(userInfo.amount.toNumber(), 0);
 
  const poolInfo = await program.account.poolInfo.fetch(program.provider.wallet.publicKey);
  assert.equal(poolInfo.amount.toNumber(), 0);
});

it("Adds a new pool", async () => {
  // Prepare test accounts
  const [owner, lpToken, lpTokenAccount] = await createTestAccounts(program);

  // Call the add_pool function
  const tx = await program.methods.addPool(
    lpToken.publicKey,
    1000, // reward_per_block
    86400, // lock_period (1 day)
    3600, // unlock_period (1 hour)
    true // emergency_enable
  )
  .accounts({
    owner: owner.publicKey,
    config: program.provider.wallet.publicKey,
    poolInfo: anchor.web3.Keypair.generate().publicKey,
    lpToken: lpTokenAccount.publicKey,
    tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
  })
  .signers([owner])
  .rpc();

  console.log("Add pool transaction signature", tx);

  // Verify the updated state
  const config = await program.account.config.fetch(program.provider.wallet.publicKey);
  assert.equal(config.poolCount, 1);

  const poolInfo = await program.account.poolInfo.fetch(
    (await anchor.web3.PublicKey.findProgramAddress(
      [anchor.utils.bytes.utf8.encode("pool-info"), program.provider.wallet.publicKey.toBuffer()],
      program.programId
    ))[0]
  );
  assert.equal(poolInfo.lpToken.toBase58(), lpToken.publicKey.toBase58());
  assert.equal(poolInfo.rewardPerBlock.toNumber(), 1000);
  assert.equal(poolInfo.lockPeriod, 86400);
  assert.equal(poolInfo.unlockPeriod, 3600);
  assert.equal(poolInfo.emergencyEnable, true);
  assert.equal(poolInfo.amount.toNumber(), 0);
}); 