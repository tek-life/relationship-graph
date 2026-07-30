import { invoke } from '@tauri-apps/api/core';

export interface DbState {
  initialized: boolean;
  hasStoredKey: boolean;
  unlocked: boolean;
}

export async function checkDbState(): Promise<DbState> {
  return invoke<DbState>('check_db_state');
}

export async function setupDatabase(password: string): Promise<void> {
  return invoke<void>('setup_database', { password });
}

export async function unlockDatabase(password: string): Promise<void> {
  return invoke<void>('unlock_database', { password });
}

export async function loadDatabaseFromKeychain(): Promise<void> {
  return invoke<void>('load_database_from_keychain');
}

export async function forgetStoredKey(): Promise<void> {
  return invoke<void>('forget_stored_key');
}
