import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation } from "@tanstack/react-query";
import { apiClient } from "@/api/client";
import { useAuthStore } from "@/store/auth";

interface LoginResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const saveAuth = useAuthStore((state) => state.login);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  const loginMutation = useMutation({
    mutationFn: async (payload: { email: string; password: string }) => {
      const { data } = await apiClient.post<LoginResponse>("/auth/login", payload);
      return data;
    },
    onSuccess: (data, variables) => {
      saveAuth(data.access_token, { email: variables.email });
      navigate("/inbox", { replace: true });
    }
  });

  const onSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    loginMutation.mutate({ email, password });
  };

  return (
    <main className="grid min-h-screen place-items-center bg-gradient-to-br from-brand-50 via-paper to-brand-100 px-4">
      <section className="w-full max-w-md rounded-2xl border border-brand-100 bg-white p-8 shadow-xl shadow-brand-100/40">
        <h1 className="text-2xl font-semibold text-ink">Sign in to RustMail</h1>
        <p className="mt-2 text-sm text-slate-600">Use your mailbox account credentials.</p>

        <form onSubmit={onSubmit} className="mt-6 space-y-4">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-slate-700">Email</span>
            <input
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none ring-brand-200 transition focus:border-brand-400 focus:ring"
              placeholder="admin@example.com"
              required
            />
          </label>

          <label className="block">
            <span className="mb-1 block text-sm font-medium text-slate-700">Password</span>
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none ring-brand-200 transition focus:border-brand-400 focus:ring"
              placeholder="********"
              required
            />
          </label>

          <button
            type="submit"
            disabled={loginMutation.isPending}
            className="w-full rounded-lg bg-brand-600 px-4 py-2 text-sm font-semibold text-white transition hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-70"
          >
            {loginMutation.isPending ? "Signing in..." : "Sign in"}
          </button>
        </form>

        {loginMutation.isError && (
          <p className="mt-4 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
            Login failed. Please check your email and password.
          </p>
        )}
      </section>
    </main>
  );
}
