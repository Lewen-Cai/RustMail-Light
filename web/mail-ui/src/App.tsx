import { ReactNode } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import LoginPage from "@/pages/Login";
import InboxPage from "@/pages/Inbox";
import { useAuthStore } from "@/store/auth";

function ProtectedRoute({ children }: { children: ReactNode }) {
  const token = useAuthStore((state) => state.token);

  if (!token) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

export default function App() {
  const token = useAuthStore((state) => state.token);

  return (
    <div className="min-h-screen bg-paper text-ink">
      <Routes>
        <Route path="/" element={<Navigate to={token ? "/inbox" : "/login"} replace />} />
        <Route path="/login" element={<LoginPage />} />
        <Route
          path="/inbox"
          element={
            <ProtectedRoute>
              <InboxPage />
            </ProtectedRoute>
          }
        />
      </Routes>
    </div>
  );
}
