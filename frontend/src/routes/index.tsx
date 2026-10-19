import { createHashRouter } from "react-router-dom";
import MainLayout from "../layouts/MainLayout/MainLayout";
import Dashboard from "../pages/Dashboard/Dashboard";
import Library from "../pages/Library/Library";
import MultiMonitor from "../pages/MultiMonitor/MultiMonitor";
import Settings from "../pages/Settings/Settings";
import Playlists from "../pages/Playlists/Playlists";
import Performance from "../pages/Performance/Performance";
import AIUpscaling from "../pages/AIUpscaling/AIUpscaling";
import Cursors from "../pages/Cursors/Cursors";
import Account from "../pages/Account/Account";

export const router =
  createHashRouter([
    {
      path: "/",
      element: <MainLayout />,

      children: [
        {
          index: true,
          element: <Dashboard />,
        },

        {
          path: "library",
          element: <Library />,
        },

        {
          path: "monitors",
          element: <MultiMonitor />,
        },

        {
          path: "settings",
          element: <Settings />,
        },

        {
          path: "playlists",
          element: <Playlists />,
        },

        {
          path: "performance",
          element: <Performance />,
        },
        {
          path: "enhance",
          element: <AIUpscaling />,
        },
        {
          path: "cursors",
          element: <Cursors />,
        },
        { path: "account", element: <Account /> },
      ],
    },
  ]);
