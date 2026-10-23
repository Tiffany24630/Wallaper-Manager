import { create } from "zustand";
import type { Playlist } from "../types/playlist";
import { playlistService } from "../services/playlistService";

interface PlaylistState {
  playlists: Playlist[];
  loading: boolean;
  error: string | null;

  loadPlaylists: () => Promise<void>;
  createPlaylist: (name: string) => Promise<void>;
  renamePlaylist: (id: string, name: string) => Promise<void>;
  deletePlaylist: (id: string) => Promise<void>;
  addWallpaperToPlaylist: (playlistId: string, wallpaperId: string) => Promise<void>;
  getPlaylistWallpapers: (playlistId: string) => Promise<string[]>;
  reorderPlaylist: (playlistId: string, wallpaperIds: string[]) => Promise<void>;
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
        throw error;
      }
    },

    renamePlaylist: async (id, name) => {
      try {
        await playlistService.rename(id, name);
        set({ playlists: await playlistService.getAll(), error: null });
      } catch (error) {
        set({ error: String(error) });
        throw error;
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
        throw error;
      }
    },

    addWallpaperToPlaylist: async (playlistId, wallpaperId) => {
      try {
        await playlistService.addWallpaper(playlistId, wallpaperId);
        const playlists = await playlistService.getAll();
        set({ playlists, error: null });
      } catch (error) {
        set({
          error: String(error),
        });
        throw error;
      }
    },

    getPlaylistWallpapers: async (playlistId) => {
      try {
        return await playlistService.getPlaylistWallpapers(playlistId);
      } catch (error) {
        set({
          error: String(error),
        });
        return [];
      }
    },

    reorderPlaylist: async (playlistId, wallpaperIds) => {
      try {
        await playlistService.reorder(playlistId, wallpaperIds);
        set({ playlists: await playlistService.getAll(), error: null });
      } catch (error) {
        set({ error: String(error) });
        throw error;
      }
    },
  }));
