import { useQuery } from "@tanstack/react-query";
import { format } from "date-fns";
import { apiClient } from "@/api/client";
import { useAuthStore } from "@/store/auth";

interface MessageItem {
  id: string;
  subject: string | null;
  from_address: string;
  received_at: string;
}

interface MessageListResponse {
  data: MessageItem[];
  limit: number;
  offset: number;
}

async function fetchMessages() {
  const { data } = await apiClient.get<MessageListResponse>("/messages", {
    params: {
      limit: 50,
      offset: 0
    }
  });

  return data;
}

export default function InboxPage() {
  const user = useAuthStore((state) => state.user);
  const logout = useAuthStore((state) => state.logout);
  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ["messages"],
    queryFn: fetchMessages
  });

  return (
    <main className="mx-auto min-h-screen w-full max-w-5xl px-4 py-8 sm:px-6">
      <header className="mb-6 flex flex-wrap items-center justify-between gap-3 rounded-2xl bg-white p-4 shadow-sm ring-1 ring-slate-200">
        <div>
          <h1 className="text-2xl font-semibold text-ink">Inbox</h1>
          <p className="text-sm text-slate-600">Signed in as {user?.email ?? "unknown user"}</p>
        </div>
        <button
          type="button"
          onClick={logout}
          className="rounded-lg border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 transition hover:border-brand-300 hover:text-brand-700"
        >
          Logout
        </button>
      </header>

      <section className="overflow-hidden rounded-2xl bg-white shadow-sm ring-1 ring-slate-200">
        <div className="border-b border-slate-200 px-4 py-3 text-sm font-medium text-slate-700">
          Recent messages
        </div>

        {isLoading && <p className="px-4 py-6 text-sm text-slate-500">Loading messages...</p>}

        {isError && (
          <div className="px-4 py-6">
            <p className="text-sm text-red-700">Failed to load messages.</p>
            <button
              type="button"
              onClick={() => refetch()}
              className="mt-3 rounded-md bg-brand-600 px-3 py-2 text-sm font-semibold text-white"
            >
              Retry
            </button>
          </div>
        )}

        {!isLoading && !isError && data?.data.length === 0 && (
          <p className="px-4 py-6 text-sm text-slate-500">No messages yet.</p>
        )}

        {!isLoading && !isError && data?.data.length ? (
          <ul className="divide-y divide-slate-200">
            {data.data.map((message) => (
              <li key={message.id} className="px-4 py-3">
                <p className="font-medium text-slate-900">{message.subject || "(No subject)"}</p>
                <p className="mt-1 text-sm text-slate-600">From: {message.from_address}</p>
                <p className="mt-1 font-mono text-xs text-slate-500">
                  {format(new Date(message.received_at), "yyyy-MM-dd HH:mm:ss")}
                </p>
              </li>
            ))}
          </ul>
        ) : null}
      </section>
    </main>
  );
}
