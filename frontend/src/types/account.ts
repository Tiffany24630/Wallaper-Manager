export type AccountUser = { id: string; email: string };
export type Device = { id: string; name: string; platform: string; last_sync_at: number | null; revoked_at: number | null };
export type Session = { token: string; expires_at: number; user: AccountUser };
