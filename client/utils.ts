/* eslint-disable @typescript-eslint/no-unsafe-assignment */
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
/* eslint-disable @typescript-eslint/no-unsafe-return */

import os from 'os';
import fs from 'mz/fs';
import path from 'path';
import yaml from 'yaml';
import {Keypair} from '@solana/web3.js';

/**
 * @private
 */
async function getConfig(): Promise<any> {
    // Path to Solana CLI config file
    const CONFIG_FILE_PATH = path.resolve(
        os.homedir(),
        '.config',
        'solana',
        'cli',
        'config.yml',
    );
    const configYml = await fs.readFile(CONFIG_FILE_PATH, {encoding: 'utf8'});
    return yaml.parse(configYml);
}

/**
 * Load and parse the Solana CLI config file to determine which RPC url to use
 */
export async function getRpcUrl(): Promise<string> {
    try {
        const config = await getConfig();
        if (!config.json_rpc_url) throw new Error('Missing RPC URL');
        return config.json_rpc_url;
    } catch (err) {
        console.warn(
            'Failed to read RPC url from CLI config file, falling back to localhost',
        );
        return 'http://localhost:8899';
    }
}

/**
 * If the lamports are in your account, you need to sign the transaction with your private key so no one else can spend your lamports.
 * This private key is stored in your local filesystem as an array of bytes.
 * The createKeypairFromFile function decodes this array and returns it as a Keypair using the fromSecretKey method provided to us by the JSON rpc API.
 * The getPayer function returns a Keypair that is debited everytime we make a transaction.
 */
/**
 * Load and parse the Solana CLI config file to determine which payer to use
 */
export async function getPayer(): Promise<Keypair> {
    try {
        const config = await getConfig();
        if (!config.keypair_path) throw new Error('Missing keypair path');
        return await createKeypairFromFile(config.keypair_path);
    } catch (err) {
        console.warn(
            'Failed to create keypair from CLI config file, falling back to new random keypair',
        );
        return Keypair.generate();
    }
}

/**
 * Create a Keypair from a secret key stored in file as bytes' array
 */
export async function createKeypairFromFile(
    filePath: string,
): Promise<Keypair> {
    const secretKeyString = await fs.readFile(filePath, {encoding: 'utf8'});
    const secretKey = Uint8Array.from(JSON.parse(secretKeyString));
    return Keypair.fromSecretKey(secretKey);
}

/**
 * In Solana, we also have to pay rent for the storage cost of keeping the account alive.
 * However, an account can be made entirely exempt from rent collection by depositing at least 2 years worth of rent.
 * The getMinimumBalanceForRentExemption API can be used to get the minimum balance required for a particular account.
 */