import { useCallback, useState } from "react";
import * as ipc from "@/lib/ipc";
import type {
  ApiKeyInfo,
  CodexRuntimeStatus,
  ProviderModel,
  ProviderAuthMode,
  ProviderType,
} from "@/types/domain";

/**
 * Hook for managing API keys per provider.
 * Wraps IPC calls to save/load/delete/validate keys.
 */
export function useApiKeys() {
  const [isLoading, setIsLoading] = useState(false);

  const load = useCallback(async (provider: ProviderType): Promise<ApiKeyInfo> => {
    setIsLoading(true);
    try {
      return await ipc.apikeyLoad(provider);
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
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const remove = useCallback(async (provider: ProviderType) => {
    setIsLoading(true);
    try {
      await ipc.apikeyDelete(provider);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const validate = useCallback(async (provider: ProviderType) => {
    setIsLoading(true);
    try {
      await ipc.apikeyValidate(provider);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const listModels = useCallback(
    async (
      provider: ProviderType,
      apiKey?: string,
      model?: string,
      endpoint?: string,
      organization?: string,
      project?: string,
      commandPath?: string,
      profile?: string,
    ): Promise<ProviderModel[]> => {
      setIsLoading(true);
      try {
        return await ipc.apikeyModels(
          provider,
          apiKey,
          model,
          endpoint,
          organization,
          project,
          commandPath,
          profile,
        );
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const inspectCodex = useCallback(
    async (
      commandPath?: string,
      model?: string,
      profile?: string,
    ): Promise<CodexRuntimeStatus> => {
      setIsLoading(true);
      try {
        return await ipc.codexStatus(commandPath, model, profile);
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
      try {
        return await ipc.codexLogin(commandPath, model, profile, deviceAuth);
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  return {
    isLoading,
    load,
    save,
    remove,
    validate,
    listModels,
    inspectCodex,
    loginCodex,
  };
}
