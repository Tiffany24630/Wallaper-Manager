import {createBrowserRouter,} from "react-router-dom";
import MainLayout from "../layouts/MainLayout/MainLayout";
import Dashboard from "../pages/Dashboard/Dashboard";
import Library from "../pages/Library/Library";
import MultiMonitor from "../pages/MultiMonitor/MultiMonitor";
import Settings from "../pages/Settings/Settings";
import Playlists from "../pages/Playlists/Playlists";
import Performance from "../pages/Performance/Performance";

export const router =
  createBrowserRouter([
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
      ],
    },
  ]);