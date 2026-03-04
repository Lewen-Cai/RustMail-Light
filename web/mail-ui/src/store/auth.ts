import { create } from "zustand";

const AUTH_STORAGE_KEY = "rustmail.auth";

export interface AuthUser {
  id?: string;
  email: string;
  role?: string;
}

interface PersistedAuthState {
  token: string;
  user: AuthUser;
}

interface AuthState {
  token: string | null;
  user: AuthUser | null;
  login: (token: string, user: AuthUser) => void;
  logout: () => void;
  restoreSession: () => void;
}

function persistSession(token: string, user: AuthUser) {
  localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify({ token, user }));
}

function clearPersistedSession() {
  localStorage.removeItem(AUTH_STORAGE_KEY);
}

function readPersistedSession(): PersistedAuthState | null {
  const raw = localStorage.getItem(AUTH_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as PersistedAuthState;
    if (!parsed.token || !parsed.user?.email) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export const useAuthStore = create<AuthState>((set) => ({
  token: null,
  user: null,
  login: (token, user) => {
    persistSession(token, user);
    set({ token, user });
  },
  logout: () => {
    clearPersistedSession();
    set({ token: null, user: null });
  },
  restoreSession: () => {
    const saved = readPersistedSession();
    if (!saved) {
      clearPersistedSession();
      return;
    }
    set({ token: saved.token, user: saved.user });
  }
}));
