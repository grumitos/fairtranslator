import { useCallback, useState } from "react";
import * as ipc from "@/lib/ipc";
import type {
  ApiKeyInfo,
  CodexRuntimeStatus,
  ProviderAuthMode,
  ProviderType,
} from "@/types/domain";

/**
 * Hook for managing API keys per provider.
 * Wraps IPC calls to save/load/delete/validate keys.
 */
export function useApiKeys() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (provider: ProviderType): Promise<ApiKeyInfo> => {
    setIsLoading(true);
    setError(null);
    try {
      return await ipc.apikeyLoad(provider);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const save = useCallback(
    async (
      provider: ProviderType,
      apiKey: string,
      model?: string,
      endpoint?: string,
      organization?: string,
      project?: string,
      commandPath?: string,
      profile?: string,
      authMode?: ProviderAuthMode,
    ) => {
      setIsLoading(true);
      setError(null);
      try {
        await ipc.apikeySave(
          provider,
          apiKey,
          model,
          endpoint,
          organization,
          project,
          commandPath,
          profile,
          authMode,
        );
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const remove = useCallback(async (provider: ProviderType) => {
    setIsLoading(true);
    setError(null);
    try {
      await ipc.apikeyDelete(provider);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const validate = useCallback(async (provider: ProviderType) => {
    setIsLoading(true);
    setError(null);
    try {
      await ipc.apikeyValidate(provider);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const inspectCodex = useCallback(
    async (
      commandPath?: string,
      model?: string,
      profile?: string,
    ): Promise<CodexRuntimeStatus> => {
      setIsLoading(true);
      setError(null);
      try {
        return await ipc.codexStatus(commandPath, model, profile);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const loginCodex = useCallback(
    async (
      commandPath?: string,
      model?: string,
      profile?: string,
      deviceAuth?: boolean,
    ): Promise<CodexRuntimeStatus> => {
      setIsLoading(true);
      setError(null);
      try {
        return await ipc.codexLogin(commandPath, model, profile, deviceAuth);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        throw e;
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  return {
    isLoading,
    error,
    load,
    save,
    remove,
    validate,
    inspectCodex,
    loginCodex,
  };
}
