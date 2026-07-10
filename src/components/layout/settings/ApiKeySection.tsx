import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useApiKeys } from "@/hooks/useApiKeys";
import type { ApiKeyInfo, CodexRuntimeStatus, ProviderModel, ProviderType } from "@/types/domain";

interface ApiKeySectionProps {
  provider: ProviderType;
}

function isConfigured(info: ApiKeyInfo): boolean {
  return (
    info.isConfigured ??
    Boolean(
      info.model ||
        info.endpoint ||
        info.organization ||
        info.project ||
        info.commandPath ||
        info.profile,
    )
  );
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
    if (!window.confirm(t("apiKeys.removeConfirm"))) {
      return;
    }
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

  return (
    <div className="space-y-5">
      <div
        className="flex items-center gap-2 rounded-md border bg-muted/35 px-3 py-2 text-xs"
        role="status"
      >
        <span
          aria-hidden="true"
          className={`size-2 rounded-full ${hasConfig ? "bg-success" : "bg-muted-foreground/45"}`}
        />
        <span className={hasConfig ? "font-medium text-success" : "text-muted-foreground"}>
          {t(
            hasConfig
              ? "apiKeys.configured"
              : isCodex
                ? "apiKeys.notConfigured"
                : "apiKeys.noKey",
          )}
        </span>
      </div>

      {!isCodex && (
        <div className="space-y-1.5">
          <Label htmlFor={`${providerKey}-api-key`}>{t("apiKeys.apiKey")}</Label>
          <Input
            id={`${providerKey}-api-key`}
            type="password"
            name={`${providerKey}-api-key`}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={
              hasConfig
                ? t("apiKeys.savedKeyPlaceholder")
                : t(`apiKeys.${providerKey}.placeholder`)
            }
          />
          {hasConfig && (
            <p className="text-xs text-muted-foreground">{t("apiKeys.savedKeyHint")}</p>
          )}
          {isOpenCode && (
            <p className="text-xs leading-5 text-muted-foreground">
              {t("apiKeys.opencode.authNote")}
            </p>
          )}
        </div>
      )}

      {supportsModel && (
        <div className="space-y-1.5">
          <div className="flex items-center justify-between gap-2">
            <Label htmlFor={`${providerKey}-model`}>
              {t("apiKeys.model")}
            </Label>
            {supportsModelRefresh && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void handleRefreshModels()}
                disabled={isLoading || isRefreshingModels || !canRefreshModels}
                className="h-7 gap-1 px-2 text-xs"
              >
                <RefreshCw className={`size-3.5 ${isRefreshingModels ? "animate-spin" : ""}`} />
                {t("apiKeys.models.refresh")}
              </Button>
            )}
          </div>
          <Input
            id={`${providerKey}-model`}
            name={`${providerKey}-model`}
            list={modelOptions.length > 0 ? `${providerKey}-models` : undefined}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={t("apiKeys.modelPlaceholder")}
          />
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
          <div className="space-y-1.5">
            <Label htmlFor="codex-command-path">{t("apiKeys.codex.commandPath")}</Label>
            <Input
              id="codex-command-path"
              name="codex-command-path"
              autoComplete="off"
              spellCheck={false}
              value={commandPath}
              onChange={(e) => setCommandPath(e.target.value)}
              placeholder={t("apiKeys.codex.commandPathPlaceholder")}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="codex-profile">{t("apiKeys.codex.profile")}</Label>
            <Input
              id="codex-profile"
              name="codex-profile"
              autoComplete="off"
              spellCheck={false}
              value={profile}
              onChange={(e) => setProfile(e.target.value)}
              placeholder={t("apiKeys.codex.profilePlaceholder")}
            />
          </div>
          <p className="text-xs leading-5 text-muted-foreground">{t("apiKeys.codex.authNote")}</p>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => void handleCodexLogin(false)}
              disabled={isLoading}
              className="text-xs"
            >
              {t("apiKeys.codex.login")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void handleCodexLogin(true)}
              disabled={isLoading}
              className="text-xs"
            >
              {t("apiKeys.codex.deviceLogin")}
            </Button>
          </div>
          {codexStatus && (
            <div className="rounded-md border bg-muted/30 p-3 text-xs">
              <p className={`font-medium ${codexStatusTone}`}>
                {t(codexStatusLabelKey ?? "apiKeys.codex.statusError")}
              </p>
              <p className="mt-1 text-muted-foreground">
                {t(
                  codexStatus.commandSource === "sidecar"
                    ? "apiKeys.codex.sourceSidecar"
                    : codexStatus.commandSource === "configured"
                      ? "apiKeys.codex.sourceConfigured"
                      : "apiKeys.codex.sourcePath",
                )}
              </p>
              {(codexStatus.resolvedCommandPath || codexStatus.commandSource !== "sidecar") && (
                <p className="text-muted-foreground">
                  {t("apiKeys.codex.resolvedPathLabel")}:{" "}
                  {codexStatus.resolvedCommandPath ?? codexStatus.effectiveCommandPath}
                </p>
              )}
              {codexStatus.version && (
                <p className="text-muted-foreground">
                  {t("apiKeys.codex.versionLabel")}: {codexStatus.version}
                </p>
              )}
              {codexStatus.state === "error" && (
                <p className="text-muted-foreground">{codexStatus.message}</p>
              )}
            </div>
          )}
        </>
      )}

      {supportsEndpoint && (
        <details className="rounded-md border bg-muted/20 px-3 py-2">
          <summary className="ft-focus -mx-1 cursor-pointer rounded px-1 text-sm font-medium text-foreground">
            {t("apiKeys.advanced")}
          </summary>
          <div className="mt-3 space-y-3 border-t pt-3">
            {provider === "openai" && (
              <>
                <div className="space-y-1.5">
                  <Label htmlFor="openai-organization">{t("apiKeys.openai.organization")}</Label>
                  <Input
                    id="openai-organization"
                    name="openai-organization"
                    autoComplete="off"
                    spellCheck={false}
                    value={organization}
                    onChange={(e) => setOrganization(e.target.value)}
                    placeholder={t("apiKeys.openai.organizationPlaceholder")}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="openai-project">{t("apiKeys.openai.project")}</Label>
                  <Input
                    id="openai-project"
                    name="openai-project"
                    autoComplete="off"
                    spellCheck={false}
                    value={project}
                    onChange={(e) => setProject(e.target.value)}
                    placeholder={t("apiKeys.openai.projectPlaceholder")}
                  />
                </div>
                <p className="text-xs leading-5 text-muted-foreground">
                  {t("apiKeys.openai.authNote")}
                </p>
              </>
            )}
            <div className="space-y-1.5">
              <Label htmlFor={`${providerKey}-endpoint`}>{t("apiKeys.endpoint")}</Label>
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
                placeholder={
                  isOpenCode
                    ? t("apiKeys.opencode.endpointPlaceholder")
                    : t("apiKeys.endpointPlaceholder")
                }
              />
            </div>
          </div>
        </details>
      )}

      <div className="flex flex-wrap gap-2 border-t pt-4">
        <Button size="sm" onClick={handleSave} disabled={!canSave || isLoading} className="text-xs">
          {t("apiKeys.save")}
        </Button>
        {showValidate && (
          <Button
            size="sm"
            variant="outline"
            onClick={handleValidate}
            disabled={isLoading}
            className="text-xs"
          >
            {isCodex ? t("apiKeys.codex.checkStatus") : t("apiKeys.validate")}
          </Button>
        )}
        {hasConfig && (
          <Button
            size="sm"
            variant="ghost"
            onClick={handleRemove}
            disabled={isLoading}
            className="text-xs text-destructive hover:text-destructive"
          >
            {t("apiKeys.remove")}
          </Button>
        )}
      </div>
    </div>
  );
}
