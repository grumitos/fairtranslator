import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useApiKeys } from "@/hooks/useApiKeys";
import type { ApiKeyInfo, CodexRuntimeStatus, ProviderModel, ProviderType } from "@/types/domain";
import { confirmAction } from "@/components/common/ConfirmDialog";

interface ApiKeySectionProps {
  provider: ProviderType;
}

function isConfigured(info: ApiKeyInfo): boolean {
  return info.isConfigured;
}

export function ApiKeySection({ provider }: ApiKeySectionProps) {
  const { t } = useTranslation("settings");
  const {
    load,
    save,
    remove,
    validate,
    listModels,
    inspectCodex,
    loginCodex,
    isLoading,
  } = useApiKeys();
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [organization, setOrganization] = useState("");
  const [project, setProject] = useState("");
  const [commandPath, setCommandPath] = useState("");
  const [profile, setProfile] = useState("");
  const [modelOptions, setModelOptions] = useState<ProviderModel[]>([]);
  const [isRefreshingModels, setIsRefreshingModels] = useState(false);
  const [hasConfig, setHasConfig] = useState(false);
  const [codexStatus, setCodexStatus] = useState<CodexRuntimeStatus | null>(null);

  const isCodex = provider === "codex";
  const isOpenCode = provider === "opencode";
  const supportsModel = provider === "openai" || provider === "codex" || isOpenCode;
  const supportsModelRefresh = provider === "openai" || provider === "codex" || isOpenCode;
  const supportsEndpoint = provider !== "codex";
  const providerKey = useMemo(() => {
    switch (provider) {
      case "openai":
        return "openai";
      case "deepl":
        return "deepl";
      case "google":
        return "google";
      case "codex":
        return "codex";
      case "opencode":
        return "opencode";
      default:
        return "openai";
    }
  }, [provider]);

  const codexStatusLabelKey = useMemo(() => {
    if (!codexStatus) return null;
    switch (codexStatus.state) {
      case "ready":
        return "apiKeys.codex.statusReady";
      case "loginRequired":
        return "apiKeys.codex.statusLoginRequired";
      case "notInstalled":
        return "apiKeys.codex.statusNotInstalled";
      default:
        return "apiKeys.codex.statusError";
    }
  }, [codexStatus]);

  const codexStatusTone = useMemo(() => {
    if (!codexStatus) return "";
    switch (codexStatus.state) {
      case "ready":
        return "text-success";
      case "loginRequired":
        return "text-warning";
      default:
        return "text-destructive";
    }
  }, [codexStatus]);

  const refreshCodexStatus = useCallback(
    async (
      nextCommandPath = commandPath,
      nextModel = model,
      nextProfile = profile,
    ) => {
      if (!isCodex) return null;
      const status = await inspectCodex(
        nextCommandPath || undefined,
        nextModel || undefined,
        nextProfile || undefined,
      );
      setCodexStatus(status);
      return status;
    },
    [commandPath, inspectCodex, isCodex, model, profile],
  );

  useEffect(() => {
    let isActive = true;
    load(provider)
      .then((info) => {
        if (!isActive) return;
        // Stored credentials stay in Rust/keyring and never enter webview state.
        setApiKey("");
        setModel(info.model ?? "");
        setEndpoint(info.endpoint ?? "");
        setOrganization(info.organization ?? "");
        setProject(info.project ?? "");
        setCommandPath(info.commandPath ?? "");
        setProfile(info.profile ?? "");
        setModelOptions([]);
        setHasConfig(isConfigured(info));

        if (provider === "codex") {
          inspectCodex(info.commandPath, info.model, info.profile)
            .then((status) => {
              if (isActive) {
                setCodexStatus(status);
              }
            })
            .catch(() => {
              if (isActive) {
                setCodexStatus(null);
              }
            });
        } else {
          setCodexStatus(null);
        }
      })
      .catch(() => {
        // Ignore load errors on mount
        if (isActive) {
          setCodexStatus(null);
        }
      });

    return () => {
      isActive = false;
    };
  }, [inspectCodex, load, provider]);

  const handleRefreshModels = useCallback(async () => {
    if (!supportsModelRefresh) return;
    setIsRefreshingModels(true);
    try {
      const models = await listModels(
        provider,
        isCodex ? undefined : apiKey || undefined,
        model || undefined,
        supportsEndpoint ? endpoint : undefined,
        provider === "openai" ? organization : undefined,
        provider === "openai" ? project : undefined,
        isCodex ? commandPath : undefined,
        isCodex ? profile : undefined,
      );
      setModelOptions(models);
      if (!model && models.length > 0) {
        setModel(models[0].id);
      }
      toast.success(t("apiKeys.models.refreshed"), {
        description: t("apiKeys.models.count", { count: models.length }),
      });
    } catch (e) {
      toast.error(t("apiKeys.models.refreshFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsRefreshingModels(false);
    }
  }, [
    apiKey,
    commandPath,
    endpoint,
    isCodex,
    listModels,
    model,
    organization,
    profile,
    project,
    provider,
    supportsEndpoint,
    supportsModelRefresh,
    t,
  ]);

  const handleSave = useCallback(async () => {
    try {
      await save(
        provider,
        isCodex ? "" : apiKey,
        model || undefined,
        supportsEndpoint ? endpoint || undefined : undefined,
        provider === "openai" ? organization || undefined : undefined,
        provider === "openai" ? project || undefined : undefined,
        isCodex ? commandPath || undefined : undefined,
        isCodex ? profile || undefined : undefined,
        isCodex ? "codexManaged" : "apiKey",
      );
      setHasConfig(
        isCodex
          ? Boolean(model || commandPath || profile)
          : hasConfig || Boolean(apiKey),
      );
      if (!isCodex) {
        setApiKey("");
      }
      if (isCodex) {
        await refreshCodexStatus(commandPath, model, profile);
      }
      toast.success(t("apiKeys.saved"));
    } catch (e) {
      toast.error(t("apiKeys.saveFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [
    apiKey,
    commandPath,
    endpoint,
    hasConfig,
    isCodex,
    model,
    organization,
    profile,
    project,
    provider,
    refreshCodexStatus,
    save,
    supportsEndpoint,
    t,
  ]);

  const handleRemove = useCallback(async () => {
    const confirmed = await confirmAction({
      message: t("apiKeys.removeConfirm"),
      confirmLabel: t("apiKeys.remove"),
      destructive: true,
    });
    if (!confirmed) return;
    try {
      await remove(provider);
      setApiKey("");
      setModel("");
      setEndpoint("");
      setOrganization("");
      setProject("");
      setCommandPath("");
      setProfile("");
      setModelOptions([]);
      setHasConfig(false);
      if (isCodex) {
        await refreshCodexStatus("", "", "");
      } else {
        setCodexStatus(null);
      }
      toast.success(t("apiKeys.removed"));
    } catch (e) {
      toast.error(t("apiKeys.removeFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  }, [isCodex, provider, refreshCodexStatus, remove, t]);

  const handleValidate = useCallback(async () => {
    try {
      if (isCodex) {
        const status = await refreshCodexStatus();
        if (status?.isAuthenticated) {
          toast.success(t("apiKeys.valid"));
        } else {
          toast.error(t("apiKeys.invalid"), {
            description: status?.message ?? t("apiKeys.codex.statusError"),
          });
        }
        return;
      }
      await validate(provider);
      toast.success(t("apiKeys.valid"));
    } catch (e) {
      toast.error(t("apiKeys.invalid"), { description: String(e) });
    }
  }, [isCodex, provider, refreshCodexStatus, t, validate]);

  const handleCodexLogin = useCallback(
    async (deviceAuth: boolean) => {
      try {
        const status = await loginCodex(
          commandPath || undefined,
          model || undefined,
          profile || undefined,
          deviceAuth,
        );
        setCodexStatus(status);
        setHasConfig(Boolean(model || commandPath || profile || status.isAuthenticated));
        if (status.isAuthenticated) {
          toast.success(t("apiKeys.codex.loginSuccess"));
        } else {
          toast.error(t("apiKeys.codex.loginIncomplete"), {
            description: status.message,
          });
        }
      } catch (e) {
        toast.error(t("apiKeys.codex.loginFailed"), {
          description: e instanceof Error ? e.message : String(e),
        });
      }
    },
    [commandPath, loginCodex, model, profile, t],
  );

  const canSave = isCodex || hasConfig || Boolean(apiKey);
  const canRefreshModels = isCodex || hasConfig || Boolean(apiKey);
  const showValidate = isCodex || hasConfig;
  const codexSourceKey = codexStatus
    ? codexStatus.commandSource === "sidecar"
      ? "apiKeys.codex.sourceShort.sidecar"
      : codexStatus.commandSource === "configured"
        ? "apiKeys.codex.sourceShort.configured"
        : "apiKeys.codex.sourceShort.path"
    : null;

  return (
    <div className="flex flex-col gap-3.5">
      {isCodex && codexStatus && (
        <dl className="grid grid-cols-[96px_minmax(0,1fr)] gap-y-2 text-xs" role="status">
          <dt className="text-ink-3">{t("apiKeys.status")}</dt>
          <dd className={`m-0 ${codexStatusTone}`}>{t(codexStatusLabelKey ?? "apiKeys.codex.statusError")}</dd>
          {codexSourceKey && (
            <>
              <dt className="text-ink-3">{t("apiKeys.codex.source")}</dt>
              <dd className="m-0 text-ink-2">{t(codexSourceKey)}</dd>
            </>
          )}
          {(codexStatus.resolvedCommandPath || codexStatus.commandSource !== "sidecar") && (
            <>
              <dt className="text-ink-3">{t("apiKeys.codex.resolvedPathLabel")}</dt>
              <dd className="m-0 truncate font-mono text-ink-2" title={codexStatus.resolvedCommandPath ?? codexStatus.effectiveCommandPath}>
                {codexStatus.resolvedCommandPath ?? codexStatus.effectiveCommandPath}
              </dd>
            </>
          )}
          {codexStatus.version && (
            <>
              <dt className="text-ink-3">{t("apiKeys.codex.versionLabel")}</dt>
              <dd className="m-0 font-mono text-ink-2">{codexStatus.version}</dd>
            </>
          )}
          {codexStatus.state === "error" && (
            <dd className="col-span-2 m-0 text-danger">{codexStatus.message}</dd>
          )}
        </dl>
      )}

      {!isCodex && (
        <div className="flex flex-col gap-1.5">
          <label htmlFor={`${providerKey}-api-key`} className="text-xs text-ink-3">
            {t("apiKeys.apiKey")}
          </label>
          <Input
            id={`${providerKey}-api-key`}
            type="password"
            name={`${providerKey}-api-key`}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={hasConfig ? t("apiKeys.savedKeyPlaceholder") : t(`apiKeys.${providerKey}.placeholder`)}
            className="font-mono text-[12.5px]"
          />
        </div>
      )}

      {supportsModel && (
        <div className="flex flex-col gap-1.5">
          <label htmlFor={`${providerKey}-model`} className="text-xs text-ink-3">
            {t("apiKeys.model")}
          </label>
          <div className="flex gap-2">
            <Input
              id={`${providerKey}-model`}
              name={`${providerKey}-model`}
              list={modelOptions.length > 0 ? `${providerKey}-models` : undefined}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder={isCodex ? t("apiKeys.codex.modelDefault") : t("apiKeys.modelPlaceholder")}
              className="font-mono text-[12.5px]"
            />
            {supportsModelRefresh && (
              <Button
                type="button"
                size="icon"
                variant="outline"
                className="size-8"
                onClick={() => void handleRefreshModels()}
                disabled={isLoading || isRefreshingModels || !canRefreshModels}
                aria-label={t("apiKeys.models.refresh")}
                title={t("apiKeys.models.refresh")}
              >
                <RefreshCw className={isRefreshingModels ? "animate-spin" : undefined} strokeWidth={1.7} />
              </Button>
            )}
          </div>
          {modelOptions.length > 0 && (
            <datalist id={`${providerKey}-models`}>
              {modelOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.ownedBy ?? ""}
                </option>
              ))}
            </datalist>
          )}
        </div>
      )}

      {isCodex && (
        <>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="codex-command-path" className="text-xs text-ink-3">
              {t("apiKeys.codex.commandPath")}
            </label>
            <Input
              id="codex-command-path"
              name="codex-command-path"
              autoComplete="off"
              spellCheck={false}
              value={commandPath}
              onChange={(e) => setCommandPath(e.target.value)}
              placeholder="PATH"
              className="font-mono text-[12.5px]"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="codex-profile" className="text-xs text-ink-3">
              {t("apiKeys.codex.profile")}
            </label>
            <Input
              id="codex-profile"
              name="codex-profile"
              autoComplete="off"
              spellCheck={false}
              value={profile}
              onChange={(e) => setProfile(e.target.value)}
              placeholder={t("apiKeys.codex.profileDefault")}
            />
          </div>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" onClick={() => void handleCodexLogin(false)} disabled={isLoading}>
              {t("apiKeys.codex.login")}
            </Button>
            <Button size="sm" variant="outline" onClick={() => void handleCodexLogin(true)} disabled={isLoading}>
              {t("apiKeys.codex.deviceLogin")}
            </Button>
          </div>
        </>
      )}

      {supportsEndpoint && (
        <details className="group">
          <summary className="flex w-fit cursor-pointer list-none items-center gap-1.5 rounded-sm text-xs text-ink-3 outline-none hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink [&::-webkit-details-marker]:hidden">
            <svg
              viewBox="0 0 24 24"
              aria-hidden="true"
              className="size-3 fill-none stroke-current transition-transform duration-(--motion-fast) group-open:rotate-90"
              strokeWidth={2}
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M9 6l6 6-6 6" />
            </svg>
            {t("apiKeys.advanced")}
          </summary>
          <div className="mt-3 flex flex-col gap-3.5">
            {provider === "openai" && (
              <>
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="openai-organization" className="text-xs text-ink-3">
                    {t("apiKeys.openai.organization")}
                  </label>
                  <Input
                    id="openai-organization"
                    name="openai-organization"
                    autoComplete="off"
                    spellCheck={false}
                    value={organization}
                    onChange={(e) => setOrganization(e.target.value)}
                    placeholder={t("apiKeys.optional")}
                    className="font-mono text-[12.5px]"
                  />
                </div>
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="openai-project" className="text-xs text-ink-3">
                    {t("apiKeys.openai.project")}
                  </label>
                  <Input
                    id="openai-project"
                    name="openai-project"
                    autoComplete="off"
                    spellCheck={false}
                    value={project}
                    onChange={(e) => setProject(e.target.value)}
                    placeholder={t("apiKeys.optional")}
                    className="font-mono text-[12.5px]"
                  />
                </div>
              </>
            )}
            <div className="flex flex-col gap-1.5">
              <label htmlFor={`${providerKey}-endpoint`} className="text-xs text-ink-3">
                {t("apiKeys.endpoint")}
              </label>
              <Input
                id={`${providerKey}-endpoint`}
                name={`${providerKey}-endpoint`}
                type="url"
                inputMode="url"
                autoComplete="off"
                autoCapitalize="none"
                spellCheck={false}
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                placeholder={isOpenCode ? t("apiKeys.opencode.endpointPlaceholder") : t("apiKeys.optional")}
                className="font-mono text-[12.5px]"
              />
            </div>
          </div>
        </details>
      )}

      <div className="flex flex-wrap items-center gap-2 pt-1">
        {showValidate && (
          <Button size="sm" variant="outline" onClick={handleValidate} disabled={isLoading}>
            {isCodex ? t("apiKeys.codex.checkStatus") : t("apiKeys.validate")}
          </Button>
        )}
        {hasConfig && (
          <Button size="sm" variant="destructive" onClick={handleRemove} disabled={isLoading}>
            {t("apiKeys.remove")}
          </Button>
        )}
        <Button size="sm" onClick={handleSave} disabled={!canSave || isLoading} className="ml-auto">
          {t("apiKeys.save")}
        </Button>
      </div>
    </div>
  );
}
