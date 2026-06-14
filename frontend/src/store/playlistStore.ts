import { create } from "zustand";

import type { Playlist } from "../types/playlist";

import { playlistService } from "../services/playlistService";

interface PlaylistState {
  playlists: Playlist[];
  loading: boolean;
  error: string | null;

  loadPlaylists: () => Promise<void>;
  createPlaylist: (name: string) => Promise<void>;
  deletePlaylist: (id: string) => Promise<void>;
}

export const usePlaylistStore =
  create<PlaylistState>((set) => ({
    playlists: [],
    loading: false,
    error: null,

    loadPlaylists: async () => {
      try {
        set({
          loading: true,
        });

        const playlists = await playlistService.getAll();

        set({
          playlists,
          loading: false,
          error: null,
        });

      } catch (error) {
        set({
          loading: false,
          error: String(error),
        });
      }
    },

    createPlaylist: async (name) => {
      try {
        set({
          loading: true,
        });

        await playlistService.create(name);
        const playlists = await playlistService.getAll();

        set({
          playlists,
          loading: false,
          error: null,
        });

      } catch (error) {
        set({
          loading: false,
          error: String(error),
        });
      }
    },

    deletePlaylist: async (id) => {
      try {
        set({
          loading: true,
        });

        await playlistService.delete(id);
        const playlists = await playlistService.getAll();

        set({
          playlists,
          loading: false,
          error: null,
        });
        
      } catch (error) {
        set({
          loading: false,
          error: String(error),
        });
      }
    },
  }));