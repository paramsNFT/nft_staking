/*
This script is for user activities
Please refer to pool.ts for pool authority activities
 */

import * as anchor from "@project-serum/anchor";
import {readFileSync} from "fs";
import {PublicKey} from "@solana/web3.js";
import {AccountInfo, Token, TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getTokenAccount, parseTokenAccount} from "@project-serum/common";

export const STAKE_PROGRAM = new anchor.web3.PublicKey(
    "paramKFFuRPLVXZWjDRbnk5xKemduYZUW2BqUp7xZD3"
);

//reward token mint
export const WILD_TOKEN = new anchor.web3.PublicKey(
    "FpXFK3tZAWNb9eQMHNFpnWH1gQ1ssmSoz7av2VYcHscw"
)
//reward token account
export const WILD_VAULT = new anchor.web3.PublicKey(
    "ABGFm7tjupN9B2LSxZLvRHLWRkjdpqBF1kzfC7qQ27wZ"
)

export const POOL_AUTHORITY = new anchor.web3.PublicKey(
    ""
)

export const POOL_ID = new anchor.web3.PublicKey(
    ""
)

export const CONFIG_ID = new anchor.web3.PublicKey(
    ""
)

const idl = JSON.parse(require('fs').readFileSync('./client/nft_staking.json', 'utf8'));

const keys_local = ""

const ENV = "devnet";


const PREFIX = "nft_staking";
const PREFIX_CONFIG = "nft_staking_config";
const PREFIX_USER = "nft_staking_user"
const PREFIX_MINT = "nft_staking_mint"

export interface Pool {
    id: anchor.web3.PublicKey,
    connection: anchor.web3.Connection;
    program: anchor.Program;
}

interface PoolState {
    isInitialized: boolean;
    authority: anchor.web3.PublicKey;
    paused: boolean;
    config: anchor.web3.PublicKey;
    rewardMint: anchor.web3.PublicKey;
    rewardVault: anchor.web3.PublicKey;
    // rewardRatePerToken: u128  // how to conver it to number
    lastUpdateTime: number,
    rewardDuration: number,
    rewardDurationEnd: number,
    tokenStakeCount: number,
    userCount: number
}

interface ConfigState {
    authority: anchor.web3.PublicKey;
    uuid: string;
    numMint: number,
    mintAddresses: anchor.web3.PublicKey[];
}

interface UserState {
    user: anchor.web3.PublicKey;
    rewardEarnedClaimed: number;
    rewardEarnedPending: number;
    mintStakedCount: number;
    uuid: string;
    mintStaked: anchor.web3.PublicKey;
    lastUpdateTime: number;
    mintStakedState: MintStakedState
}

interface MintStakedState {
    userAccount: anchor.web3.PublicKey; // user account address
    mintAccounts: anchor.web3.PublicKey[];
}

export interface TokenInfo {
    account: anchor.web3.PublicKey;
    mint: anchor.web3.PublicKey,
}

export const getPoolState = async (
    program: anchor.Program,
    poolId: anchor.web3.PublicKey,  // pool account public key
): Promise<PoolState | null> => {
    let state = await program.account.pool.fetch(poolId);

    if (state == null) {
        return null;
    }
    let isInitialized = state.isInitialized;
    let authority = state.authority;
    let paused = state.paused;
    let config = state.config;
    let rewardMint = state.rewardMint
    let rewardVault = state.rewardVault
    let lastUpdateTime = state.lastUpdateTime.toNumber()
    let rewardDuration = state.rewardDuration.toNumber();
    let rewardDurationEnd = state.rewardDurationEnd.toNumber();
    let tokenStakeCount = state.tokenStakeCount;
    let userCount = state.userCount;
    return {
        isInitialized,
        authority,
        paused,
        config,
        rewardMint,
        rewardVault,
        lastUpdateTime,
        rewardDuration,
        rewardDurationEnd,
        tokenStakeCount,
        userCount
    }
}

export const getConfigState = async (
    program: anchor.Program,
    configId: anchor.web3.PublicKey,  // pool account public key
): Promise<ConfigState | null> => {
    let state = await program.account.config.fetch(configId);

    if (state == null) {
        return null;
    }
    let authority = state.authority;
    let uuid = state.uuid;
    let numMint = state.numMint;
    let mintAddresses = state.mintAddresses.map((element) => element)
    return {
        authority,
        uuid,
        numMint,
        mintAddresses
    }
}

export const getUserState = async (
    program: anchor.Program,
    userAccountId: anchor.web3.PublicKey,  // pool account public key
): Promise<UserState | null> => {
    let state = await program.account.user.fetch(userAccountId);

    if (state == null) {
        return null;
    }
    let mintStakedRes = await program.account.mintStaked.fetch(state.mintStaked);
    if (mintStakedRes == null) {
        return null;
    }

    let user = state.user;
    let rewardEarnedClaimed = state.rewardEarnedClaimed.toNumber();
    let rewardEarnedPending = state.rewardEarnedPending.toNumber();
    let mintStakedCount = state.mintStakedCount;
    let uuid = state.uuid;
    let mintStaked = state.mintStaked;
    let lastUpdateTime = state.lastUpdateTime.toNumber();

    let userAccount = mintStakedRes.userAccount
    let mintAccounts = mintStakedRes.mintAccounts

    let mintStakedState: MintStakedState = {
        userAccount: userAccount,
        mintAccounts: []
    }
    mintAccounts.forEach((e) => mintStakedState.mintAccounts.push(e));

    return {
        user,
        rewardEarnedClaimed,
        rewardEarnedPending,
        mintStakedCount,
        uuid,
        mintStaked,
        lastUpdateTime,
        mintStakedState
    }

}

const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: PublicKey = new PublicKey(
    'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL',
);

async function findAssociatedTokenAddress(
    walletAddress: PublicKey,
    tokenMintAddress: PublicKey
): Promise<PublicKey> {
    return (await PublicKey.findProgramAddress(
        [
            walletAddress.toBuffer(),
            TOKEN_PROGRAM_ID.toBuffer(),
            tokenMintAddress.toBuffer(),
        ],
        SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID
    ))[0];
}

async function getOrCreateAssociatedTokenAccountInfo(
    provider: anchor.Provider,
    connection: anchor.web3.Connection,
    wallet: anchor.web3.Keypair,
    tokenMintAddress: PublicKey
): Promise<AccountInfo> {
    let rewardMint = new Token(connection,
        tokenMintAddress,
        TOKEN_PROGRAM_ID,
        wallet);
    return await rewardMint.getOrCreateAssociatedAccountInfo(
        wallet.publicKey
    )
}


const generateUuid = (): string => {
    return anchor.web3.Keypair.generate()
        .publicKey.toBase58()
        .slice(0, 6);
}

const getConfigAccount = async (
    authorityId: anchor.web3.PublicKey,
    configUuid: string,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX_CONFIG)),
                authorityId.toBuffer(),
                Buffer.from(configUuid)
            ],
            STAKE_PROGRAM
        )
    );
};

const getPoolAccount = async (
    authorityId: anchor.web3.PublicKey,
    configPubKey: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX)),
                authorityId.toBuffer(),
                configPubKey.toBuffer()
            ],
            STAKE_PROGRAM
        )
    );
};

const getRewardAccount = async (
    authorityId: anchor.web3.PublicKey,
    poolId: anchor.web3.PublicKey,
    rewardMintId: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX)),
                poolId.toBuffer(),
                authorityId.toBuffer(),
                rewardMintId.toBuffer(),],
            STAKE_PROGRAM
        )
    );
};

const getUserAccount = async (
    poolAccount: anchor.web3.PublicKey,
    userWallet: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX_USER)),
                poolAccount.toBuffer(),
                userWallet.toBuffer(),
            ],
            STAKE_PROGRAM
        )
    );
};

const getMintStakedAccount = async (
    poolPda: anchor.web3.PublicKey,
    userAccount: anchor.web3.PublicKey,
    uuid: string
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX_MINT)),
                poolPda.toBuffer(),
                userAccount.toBuffer(),
                Buffer.from(uuid)
            ],
            STAKE_PROGRAM
        )
    );
};

const getTokensByUser = async (
    connection: anchor.web3.Connection,
    user: anchor.web3.PublicKey,
): Promise<TokenInfo[]> => {
    let tokenAccountsInfo = await connection.getTokenAccountsByOwner(user, {programId: TOKEN_PROGRAM_ID});
    let tokenAccounts = []

    if (tokenAccountsInfo.value.length > 0) {
        console.log('User has tokens in their wallet')

        for (let i = 0; i < tokenAccountsInfo.value.length; i++) {
            let res = await parseTokenAccount(tokenAccountsInfo.value[i].account.data)
            if (res != null) {
                let mintAddress = res.mint
                tokenAccounts.push({
                    "account": tokenAccountsInfo.value[i].pubkey,
                    "mint": mintAddress
                })
            }
        }
    }
    return tokenAccounts
};

export const createUser = async (
    program: anchor.Program,
    userWallet: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
): Promise<any> => {
    let [userPda, userBump] = await getUserAccount(
        poolAccount,
        userWallet.publicKey,
    )
    let mintStakedUuid = generateUuid();

    let [userMintStakedAccount, userMintStakedBump] = await getMintStakedAccount(
        poolAccount,
        userPda,
        mintStakedUuid
    )

    return await program.rpc.createUser(
        userBump,
        userMintStakedBump,
        mintStakedUuid,
        {
            accounts: {
                user: userWallet.publicKey,
                poolAccount: poolAccount,
                userAccount: userPda,
                mintStaked: userMintStakedAccount,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                systemProgram: anchor.web3.SystemProgram.programId,
            },
            signers: [userWallet],
        });
}

export const closeUser = async (
    program: anchor.Program,
    userWallet: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,
    userAccount: anchor.web3.PublicKey
): Promise<any> => {
    return await program.rpc.closeUser(
        {
            accounts: {
                user: userWallet.publicKey,
                poolAccount: poolAccount,
                userAccount: userAccount,
            },
            signers: [userWallet],
        });
}

export const stake = async (
    program: anchor.Program,
    userWallet: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,
    configAccount: anchor.web3.PublicKey,
    userAccount: anchor.web3.PublicKey,
    stakeFromAccount: anchor.web3.PublicKey,
    currentMintStakedAccount: anchor.web3.PublicKey
): Promise<any> => {

    let mintStakedUuid = generateUuid();

    let [userMintStakedAccount, userMintStakedBump] = await getMintStakedAccount(
        poolAccount,
        userAccount,
        mintStakedUuid
    )

    return program.rpc.stake(
        userMintStakedBump,
        mintStakedUuid,
        {
            accounts: {
                staker: userWallet.publicKey,
                poolAccount: poolAccount,
                config: configAccount,
                authority: POOL_AUTHORITY,
                userAccount: userAccount,
                stakeFromAccount: stakeFromAccount,
                mintStaked: userMintStakedAccount,
                currentMintStaked: currentMintStakedAccount,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                tokenProgram: TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            },
            signers: [userWallet],
        });
}

export const unstake = async (
    program: anchor.Program,
    userWallet: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,
    configAccount: anchor.web3.PublicKey,
    userAccount: anchor.web3.PublicKey,
    unstakeFromAccount: anchor.web3.PublicKey,
    currentMintStakedAccount: anchor.web3.PublicKey
): Promise<any> => {

    let mintStakedUuid = generateUuid();

    let [userMintStakedAccount, userMintStakedBump] = await getMintStakedAccount(
        poolAccount,
        userAccount,
        mintStakedUuid
    )

    return program.rpc.unstake(
        userMintStakedBump,
        mintStakedUuid,
        {
            accounts: {
                staker: userWallet.publicKey,
                poolAccount: poolAccount,
                config: configAccount,
                authority: POOL_AUTHORITY,
                userAccount: userAccount,
                unstakeFromAccount: unstakeFromAccount,
                mintStaked: userMintStakedAccount,
                currentMintStaked: currentMintStakedAccount,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                tokenProgram: TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            },
            signers: [userWallet],
        });
}

export const claim = async (
    program: anchor.Program,
    userWallet: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,
    rewardAccount: anchor.web3.PublicKey,
    userAccount: anchor.web3.PublicKey,
    rewardToAccount: anchor.web3.PublicKey,
): Promise<any> => {

    return program.rpc.claim(
        {
            accounts: {
                user: userWallet.publicKey,
                poolAccount: poolAccount,
                authority: POOL_AUTHORITY,
                rewardVault: rewardAccount,
                userAccount: userAccount,
                rewardToAccount: rewardToAccount,
                tokenProgram: TOKEN_PROGRAM_ID,
            },
            signers: [userWallet],
        });
}

(async () => {
    const solConnection = new anchor.web3.Connection(
        `https://api.${ENV}.solana.com/`,
    );
    const walletKey = anchor.web3.Keypair.fromSecretKey(
        new Uint8Array(JSON.parse(readFileSync(keys_local).toString())),
    );
    const walletWrapper = new anchor.Wallet(walletKey);
    const provider = new anchor.Provider(solConnection, walletWrapper, {
        preflightCommitment: 'recent',
    });
    console.log("idl")

    const anchorProgram = new anchor.Program(idl, STAKE_PROGRAM, provider);

    // get pool state
    console.log("pool")
    console.log(anchorProgram)
    let poolState = await getPoolState(anchorProgram, POOL_ID)
    console.assert(poolState.isInitialized == true)
    console.assert(poolState.paused == false)
    console.log(poolState)

    // get config state
    let configState = await getConfigState(anchorProgram, CONFIG_ID)
    console.assert(configState.numMint == configState.mintAddresses.length)
    console.log('Number of NFTs:', configState.numMint)
    //console.log(configState)

    // check how much reward in the pool
    let rewardVaultInfo = await getTokenAccount(provider, poolState.rewardVault);
    console.log('Reward amount: ', rewardVaultInfo.amount.toNumber())

    /// user activities
    // check tokens held by user
    let tokenAccounts = await getTokensByUser(solConnection, walletKey.publicKey)
    console.log(tokenAccounts)
    console.log(tokenAccounts[0]["mint"].toBase58())


    // check if token in config
    // if (tokenAccounts.length > 0) {
    //     console.log(configState.mintAddresses.some(e => e.toBase58() === tokenAccounts[0].mint.toBase58()))
    // }

    // prepare relevant keys

    let [poolAccount, _poolBump] = await getPoolAccount(POOL_AUTHORITY, CONFIG_ID);
    let [rewardAccount, _rewardBump] = await getRewardAccount(POOL_AUTHORITY, poolAccount, WILD_TOKEN);

    // create a user account in pool -- only run once

    await createUser(
        anchorProgram,
        walletKey,
        poolAccount // poolPda, not pool id
    )

    // get user account info
    let [userAccount, _] = await getUserAccount(
        poolAccount,
        walletKey.publicKey,
    )
    console.log(userAccount)

    let userState = await getUserState(
        anchorProgram,
        userAccount
    )
    console.log('User State before stake: ', userState)

    // user stake one -- only run once
    let stakeFromAccount = tokenAccounts[1].account
    await stake(
        anchorProgram,
        walletKey,
        poolAccount,
        CONFIG_ID,
        userAccount,
        stakeFromAccount,
        userState.mintStaked
    )

    userState = await getUserState(
        anchorProgram,
        userAccount
    )
    // console.log('User State after stake: ', userState)
    // console.log(userState.mintStakedState.mintAccounts[0])

    // // update user tokens information
    // tokenAccounts = await getTokensByUser(solConnection, walletKey.publicKey)
    // console.log(tokenAccounts.length, ' tokens in user wallet: ', tokenAccounts)

    // await sleep(5000);
    // user unstake
    await unstake(
        anchorProgram,
        walletKey,
        poolAccount,
        CONFIG_ID,
        userAccount,
        userState.mintStakedState.mintAccounts[0],
        userState.mintStaked
    )

    // get user state
    userState = await getUserState(
        anchorProgram,
        userAccount
    )
    // console.log('User State after unstake: ', userState)

    // update user tokens information
    // tokenAccounts = await getTokensByUser(solConnection, walletKey.publicKey)
    // console.log(tokenAccounts.length, ' tokens in user wallet: ', tokenAccounts)

    // user claim
    // create or get an associate wallet of WILD for user
    let rewardToAccountInfo = await getOrCreateAssociatedTokenAccountInfo(
        provider,
        solConnection,
        walletKey,
        WILD_TOKEN
    )

    // console.log(rewardToAccountInfo)
    // await sleep(1000)
    // //console.log('User pending reward:', (await getPendingRewardsFunction(anchorProgram, poolAccount, userAccount)).toNumber());
    // await sleep(1000)
    // //console.log('User pending reward:', (await getPendingRewardsFunction(anchorProgram, poolAccount, userAccount)).toNumber());
    // await sleep(1000)
    // let [userAccount2, _1] = await getUserAccount(
    //     poolAccount,
    //     new anchor.web3.PublicKey("")
    // )
    // console.log('User pending reward:', (await getPendingRewardsFunction(anchorProgram,
    //      poolAccount, userAccount2)).toNumber());
    // await claim(
    //     anchorProgram,
    //     walletKey,
    //     poolAccount,
    //     rewardAccount,
    //     userAccount,
    //     rewardToAccount
    // )

    // check account balance
    // rewardToAccountInfo = await getTokenAccount(provider, rewardToAccountInfo.address);
    // console.log('Reward amount in user wallet: ', rewardToAccountInfo.amount.toNumber())

})();

const sleep = (ms: number): Promise<void> => {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

async function getPendingRewardsFunction(rewardsPoolAnchorProgram, poolPubkey, userAccountPubkey) {

    const U64_MAX = new anchor.BN("18446744073709551615", 10);
    let poolObject = await rewardsPoolAnchorProgram.account.pool.fetch(poolPubkey);
    let userObject = await rewardsPoolAnchorProgram.account.user.fetch(userAccountPubkey);
    let rewardRatePerToken = poolObject.rewardRatePerToken;
    let mintStakedCount = new anchor.BN(userObject.mintStakedCount);
    let rewardEarnedPending = userObject.rewardEarnedPending;
    let lastUpdate = userObject.lastUpdateTime;

    let rewardBalance = await rewardsPoolAnchorProgram.provider.connection.getTokenAccountBalance(poolObject.rewardVault);
    // rewardBalance = new anchor.BN(parseInt(rewardBalance.value.amount));

    let elapsed = new anchor.BN(Math.max(Math.floor(Date.now() / 1000) - lastUpdate, 0));
    console.log("get pending rewards")
    console.log(rewardRatePerToken.div(U64_MAX).toNumber())
    console.log(U64_MAX)
    console.log(mintStakedCount.toNumber())
    console.log(elapsed.toNumber())
    console.log(rewardEarnedPending.toNumber())

    return rewardRatePerToken.div(U64_MAX).mul(mintStakedCount).mul(elapsed).add(rewardEarnedPending)
}