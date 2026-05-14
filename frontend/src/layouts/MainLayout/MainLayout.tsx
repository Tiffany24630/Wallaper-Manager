import { ReactNode } from "react";
import Sidebar from "../../components/common/Sidebar/Sidebar";

interface Props {
  children: ReactNode;
}

export default function MainLayout({ children }: Props) {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-text">
      <Sidebar />

      <main className="flex-1 overflow-y-auto p-8 bg-gradient-to-br from-background via-surface to-card">
        {children}
      </main>
    </div>
  );
}