import { ReactNode } from "react";
import Sidebar from "../../components/common/Sidebar/Sidebar";

interface Props {
  children: ReactNode;
}

export default function MainLayout({ children }: Props) {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-zinc-950">
      <Sidebar />

      <main className="flex-1 overflow-y-auto bg-zinc-900 p-8">
        {children}
      </main>
    </div>
  );
}