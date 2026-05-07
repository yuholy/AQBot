import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { invoke, isTauri } from '@/lib/invoke';

export type AvatarType = 'icon' | 'emoji' | 'url' | 'file';

interface UserProfile {
  name: string;
  avatarType: AvatarType;
  avatarValue: string;
}

interface UserProfileState {
  profile: UserProfile;
  loaded: boolean;
  loadProfile: () => Promise<void>;
  updateProfile: (partial: Partial<UserProfile>) => Promise<void>;
  saveAvatarFile: (dataUri: string) => Promise<void>;
}

export const useUserProfileStore = create<UserProfileState>()(
  persist(
    (set, get) => ({
      profile: {
        name: '',
        avatarType: 'icon',
        avatarValue: '',
      },
      loaded: false,
      loadProfile: async () => {
        if (!isTauri()) {
          set({ loaded: true });
          return;
        }
        try {
          const localProfile = get().profile;
          const profile = await invoke<UserProfile>('get_user_profile');
          const remoteEmpty =
            !profile.name &&
            profile.avatarType === 'icon' &&
            !profile.avatarValue;
          const localHasData =
            !!localProfile.name ||
            localProfile.avatarType !== 'icon' ||
            !!localProfile.avatarValue;

          if (remoteEmpty && localHasData) {
            set({ loaded: true });
            await get().updateProfile(localProfile);
            return;
          }

          set({ profile, loaded: true });
        } catch {
          set({ loaded: true });
        }
      },
      updateProfile: async (partial) => {
        const profile = { ...get().profile, ...partial };
        set({ profile });
        if (!isTauri()) return;
        try {
          await invoke('update_user_profile', { profile });
        } catch {
          // Keep local state even if persistence fails.
        }
      },
      saveAvatarFile: async (dataUri: string) => {
        const match = dataUri.match(/^data:([^;]+);base64,(.+)$/s);
        if (!match) throw new Error('Invalid data URI');
        await get().updateProfile({
          avatarType: 'file',
          avatarValue: dataUri,
        });
      },
    }),
    { name: 'aqbot_user_profile' },
  ),
);
