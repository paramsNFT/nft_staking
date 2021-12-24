/*
This script is for pool initialization and activities
 */

import * as anchor from "@project-serum/anchor";
import {readFileSync} from "fs";
import {PublicKey} from "@solana/web3.js";
import {TOKEN_PROGRAM_ID} from "@solana/spl-token";
import {getTokenAccount} from "@project-serum/common";
import {CreateAccountWithSeedParams} from "@solana/web3.js";
import {sendAndConfirmTransaction} from "@solana/web3.js";

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

const REWARD_DURATION = 31536000;
export const NFT_ADDRESSES = JSON.parse(require('fs').readFileSync('./client/token_mints.json', 'utf8'));
export const NUM_NFT = NFT_ADDRESSES.length;
const idl = JSON.parse(require('fs').readFileSync('./client/nft_staking.json', 'utf8'));

const keys_local = ""

const ENV = "mainnet-beta";


const PREFIX = "nft_staking";
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

export const getPoolState = async (
    program: anchor.Program,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
): Promise<PoolState | null> => {
    let state = await program.account.pool.fetch(poolAccount);

    if (state == null) {
        return null;
    }
    console.log(state)
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
    configAccount: anchor.web3.PublicKey,  // pool account public key
): Promise<ConfigState | null> => {
    let state = await program.account.config.fetch(configAccount);

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

const generateUuid = (): string => {
    return anchor.web3.Keypair.generate()
        .publicKey.toBase58()
        .slice(0, 6);
}

// derive a config account that falls off the ed25519 curve
const getConfigAccount = async (
    authority: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, string]> => {

    let configUuid = generateUuid()

    let configAccount = await PublicKey.createWithSeed(
        authority,
        configUuid,
        STAKE_PROGRAM,
    );

    while (PublicKey.isOnCurve(configAccount.toBuffer())) {
        configUuid = generateUuid()
        configAccount = await PublicKey.createWithSeed(
            authority,
            configUuid,
            STAKE_PROGRAM,
        );
    }

    return [configAccount, configUuid]
};

const getPoolAccount = async (
    authority: anchor.web3.PublicKey,
    configPubKey: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [
                Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX)),
                authority.toBuffer(),
                configPubKey.toBuffer()
            ],
            STAKE_PROGRAM
        )
    );
};

const getRewardAccount = async (
    authority: anchor.web3.PublicKey,
    poolAccount: anchor.web3.PublicKey,
    rewardMintId: anchor.web3.PublicKey,
): Promise<[anchor.web3.PublicKey, number]> => {
    return (
        await anchor.web3.PublicKey.findProgramAddress(
            [Buffer.from(anchor.utils.bytes.utf8.encode(PREFIX)),
                poolAccount.toBuffer(),
                authority.toBuffer(),
                rewardMintId.toBuffer(),],
            STAKE_PROGRAM
        )
    );
};

export const initializePool = async (
    program: anchor.Program,
    authority: anchor.web3.Keypair,
    rewardMint: anchor.web3.PublicKey,
    numNFT: number,
    rewardDuration: number,
    connection: anchor.web3.Connection,
    configAccount: anchor.web3.PublicKey,
    configUuid: string,
    poolAccount: anchor.web3.PublicKey,
    poolBump: number,
    rewardAccount: anchor.web3.PublicKey,
    rewardBump: number
): Promise<any> => {

    let configSpace = (8 + // discriminator
            32 + // authority
            4 + 6 + // uuid + u32 le
            4 + // num_mint
            4) // u32 len for Vec<Pubkey>
        +
        (32 * numNFT)

    let initPoolTx = program.transaction.initializePool(
        poolBump,
        configUuid,
        new anchor.BN(numNFT),
        rewardBump,
        new anchor.BN(rewardDuration),
        {
            accounts: {
                authority: authority.publicKey, // owner wallet
                poolAccount: poolAccount, // Pool Account
                config: configAccount,
                rewardMint: rewardMint,
                rewardVault: rewardAccount,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                tokenProgram: TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            },
            instructions: [
                anchor.web3.SystemProgram.createAccountWithSeed(<CreateAccountWithSeedParams>{
                    fromPubkey: authority.publicKey,
                    newAccountPubkey: configAccount,
                    basePubkey: authority.publicKey,
                    seed: configUuid,
                    lamports: await connection.getMinimumBalanceForRentExemption(
                        configSpace
                    ),
                    space: configSpace,
                    programId: STAKE_PROGRAM,
                })
            ]

        }
    )
    console.log(initPoolTx)
    let transactions = new anchor.web3.Transaction().add(initPoolTx)
    return await sendAndConfirmTransaction(
        connection,
        transactions,
        [authority]
    );

}

export const resumePool = async (
    program: anchor.Program,
    authority: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
): Promise<any> => {
    // set pool paused to false
    return await program.rpc.resume(
        {
            accounts: {
                authority: authority.publicKey, // owner wallet
                poolAccount: poolAccount, // Pool Account
            },
            signers: [authority]
        }
    );
}

export const authorizeFunder = async (
    program: anchor.Program,
    authority: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
    funder: anchor.web3.PublicKey,  // pool account public key
): Promise<any> => {
    return await program.rpc.authorizeFunder(
        funder,
        {
            accounts: {
                authority: authority.publicKey,
                poolAccount: poolAccount,
            },
            signers: [authority],
        });
}

export const fund = async (
    program: anchor.Program,
    funder: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
    configAccount: anchor.web3.PublicKey,  // config account public key
    rewardVault: anchor.web3.PublicKey,  // reward vault
    funderVault: anchor.web3.PublicKey,  // funder vault
    authority: anchor.web3.PublicKey, // authority
    amount: number,
): Promise<any> => {
    return await program.rpc.fund(
        new anchor.BN(amount),
        {
            accounts: {
                funder: funder.publicKey,
                poolAccount: poolAccount,
                rewardVault: rewardVault,
                funderVault: funderVault,
                authority: authority,
                tokenProgram: TOKEN_PROGRAM_ID,
                config: configAccount
            },
            signers: [funder],
        });
}

export const addMintAddresses = async (
    program: anchor.Program,
    authority: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
    configAccount: anchor.web3.PublicKey,  // config public key
    mintAddresses: string[],
) => {
    let batchSize = 20
    // do small batch with 10 in each
    if (mintAddresses.length < batchSize) {
        await program.rpc.addMintAddresses(
            mintAddresses.map((element) => new anchor.web3.PublicKey(element)),
            0,
            {
                accounts: {
                    authority: authority.publicKey, // owner wallet
                    poolAccount: poolAccount, // Pool Account
                    config: configAccount, // config account
                },
                signers: [authority]
            }
        );
    } else {
        let start = 0
        do {
            let mintAddressesBatch = mintAddresses.slice(start, start + batchSize);
            console.log("adding mintAddress:", start)
            // console.log(mintAddressesBatch.map((element) => new anchor.web3.PublicKey(element)))
            await program.rpc.addMintAddresses(
                mintAddressesBatch.map((element) => new anchor.web3.PublicKey(element)),
                start,
                {
                    accounts: {
                        authority: authority.publicKey, // owner wallet
                        poolAccount: poolAccount, // Pool Account
                        config: configAccount, // config account
                    },
                    signers: [authority]
                }
            );
            start = start + batchSize;
        } while ((start) < mintAddresses.length);
    }
}

export const closePool = async (
    program: anchor.Program,
    authority: anchor.web3.Keypair,
    poolAccount: anchor.web3.PublicKey,  // pool account public key
    configAccount: anchor.web3.PublicKey,  // config account public key
    refundee: anchor.web3.PublicKey,  // lamports refund to
    rewardRefundee: anchor.web3.PublicKey, // reward refund to
    rewardVault: anchor.web3.PublicKey,  // reward vault
): Promise<any> => {
    return await program.rpc.closePool(
        {
            accounts: {
                authority: authority.publicKey,
                poolAccount: poolAccount,
                config: configAccount,
                refundee: refundee,
                rewardRefundee: rewardRefundee,
                rewardVault: rewardVault,
                tokenProgram: TOKEN_PROGRAM_ID,
            },
            signers: [authority],
        });
}

(async () => {
    // const solConnection = new anchor.web3.Connection(
    //     `http://127.0.0.1:8899`,
    // );
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

    // const idl = await anchor.Program.fetchIdl(STAKE_PROGRAM, provider);
    const anchorProgram = new anchor.Program(idl, STAKE_PROGRAM, provider);

    console.log("Number of NFTs:", NUM_NFT)
    console.log("Reward Duration:", REWARD_DURATION)

    let [configAccount, configUuid] = await getConfigAccount(walletKey.publicKey);
    let [poolAccount, poolBump] = await getPoolAccount(walletKey.publicKey, configAccount);
    let [rewardAccount, rewardBump] = await getRewardAccount(walletKey.publicKey, poolAccount, WILD_TOKEN);


    // initialize pool - only run once
    let res = await initializePool(
        anchorProgram,
        walletKey,
        WILD_TOKEN,
        NUM_NFT,
        REWARD_DURATION,
        solConnection,
        configAccount,
        configUuid,
        poolAccount,
        poolBump,
        rewardAccount,
        rewardBump
    )
    console.log(res)

    // get pool state
    let poolState = await getPoolState(anchorProgram, poolAccount)
    console.assert(poolState.isInitialized == true)
    console.assert(poolState.paused == true)
    // console.log(poolState)

    let configState = await getConfigState(anchorProgram, configAccount)
    let storedNFTs = configState.mintAddresses.map(element => element.toBase58())
    let nftToUpload = NFT_ADDRESSES.filter((element) => !storedNFTs.includes(element))

    // console.log(configState)

    // upload mint addresses to config - only run once
    await addMintAddresses(
        anchorProgram,
        walletKey,
        poolAccount, //POOL_ID,
        configAccount, //CONFIG_ID,
        nftToUpload
    )

    // start the pool after upload config - only run once
    await resumePool(anchorProgram, walletKey, poolAccount)//POOL_ID)

    // get config state

    // console.assert(configState.numMint == configState.mintAddresses.length)
    // console.log('Number of NFTs:', configState.numMint)
    // console.log(configState.mintAddresses[0].toBase58())

    //fund pool, authority == funder here  - only run once
    await fund(
        anchorProgram,
        walletKey,
        poolAccount, 
        configAccount, 
        poolState.rewardVault,
        WILD_VAULT,
        walletKey.publicKey,
        5_000_000_000_000_000
    );
    console.log("Config Account:", configAccount.toBase58())
    console.log("Pool Account:", poolAccount.toBase58())
    console.log("rewardAccount:", rewardAccount.toBase58())
    // // get reward vault info
    // let wildMint = await getMintInfo(provider, WILD_TOKEN);
    let rewardVaultInfo = await getTokenAccount(provider, poolState.rewardVault);
    // console.log(wildMint)
    console.log("reward vault amount:", rewardVaultInfo.amount.toNumber())

})();



