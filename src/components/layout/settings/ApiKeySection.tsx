import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useApiKeys } from "@/hooks/useApiKeys";
import type { ApiKeyInfo, CodexRuntimeStatus, ProviderType } from "@/types/domain";

interface ApiKeySectionProps {
  provider: ProviderType;
}

function isConfigured(info: ApiKeyInfo): boolean {
  return (
    info.isConfigured ??
    Boolean(
      info.apiKey ||
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
  const { load, save, remove, validate, inspectCodex, loginCodex, isLoading } = useApiKeys();
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [organization, setOrganization] = useState("");
  const [project, setProject] = useState("");
  const [commandPath, setCommandPath] = useState("");
  const [profile, setProfile] = useState("");
  const [hasConfig, setHasConfig] = useState(false);
  const [codexStatus, setCodexStatus] = useState<CodexRuntimeStatus | null>(null);

  const isCodex = provider === "codex";
  const supportsModel = provider === "openai" || provider === "codex";
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
        setApiKey(info.apiKey);
        setModel(info.model ?? "");
        setEndpoint(info.endpoint ?? "");
        setOrganization(info.organization ?? "");
        setProject(info.project ?? "");
        setCommandPath(info.commandPath ?? "");
        setProfile(info.profile ?? "");
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
          : Boolean(apiKey || model || endpoint || organization || project),
      );
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
    try {
      await remove(provider);
      setApiKey("");
      setModel("");
      setEndpoint("");
      setOrganization("");
      setProject("");
      setCommandPath("");
      setProfile("");
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

  const canSave = isCodex || Boolean(apiKey);
  const showValidate = isCodex || hasConfig;

  return (
    <div className="space-y-3 rounded-lg border p-3">
      <div className="flex items-center justify-between">
        <Label className="text-sm font-medium">{t(`apiKeys.${providerKey}.label`)}</Label>
        {hasConfig && (
          <span className="text-xs text-success">
            {t("apiKeys.configured")}
          </span>
        )}
      </div>

      {!isCodex && (
        <Input
          type="password"
          name={`${providerKey}-api-key`}
          aria-label={`${t(`apiKeys.${providerKey}.label`)} ${t("apiKeys.apiKey")}`}
          autoComplete="off"
          spellCheck={false}
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder={t(`apiKeys.${providerKey}.placeholder`)}
          className="text-xs"
        />
      )}

      {supportsModel && (
        <Input
          name={`${providerKey}-model`}
          aria-label={t("apiKeys.model")}
          autoComplete="off"
          spellCheck={false}
          value={model}
          onChange={(e) => setModel(e.target.value)}
          placeholder={t("apiKeys.modelPlaceholder")}
          className="text-xs"
        />
      )}

      {provider === "openai" && (
        <>
          <Input
            name="openai-organization"
            aria-label={t("apiKeys.openai.organization")}
            autoComplete="off"
            spellCheck={false}
            value={organization}
            onChange={(e) => setOrganization(e.target.value)}
            placeholder={t("apiKeys.openai.organizationPlaceholder")}
            className="text-xs"
          />
          <Input
            name="openai-project"
            aria-label={t("apiKeys.openai.project")}
            autoComplete="off"
            spellCheck={false}
            value={project}
            onChange={(e) => setProject(e.target.value)}
            placeholder={t("apiKeys.openai.projectPlaceholder")}
            className="text-xs"
          />
          <p className="text-xs text-muted-foreground">{t("apiKeys.openai.authNote")}</p>
        </>
      )}

      {isCodex && (
        <>
          <Input
            name="codex-command-path"
            aria-label={t("apiKeys.codex.commandPath")}
            autoComplete="off"
            spellCheck={false}
            value={commandPath}
            onChange={(e) => setCommandPath(e.target.value)}
            placeholder={t("apiKeys.codex.commandPathPlaceholder")}
            className="text-xs"
          />
          <Input
            name="codex-profile"
            aria-label={t("apiKeys.codex.profile")}
            autoComplete="off"
            spellCheck={false}
            value={profile}
            onChange={(e) => setProfile(e.target.value)}
            placeholder={t("apiKeys.codex.profilePlaceholder")}
            className="text-xs"
          />
          <p className="text-xs text-muted-foreground">{t("apiKeys.codex.authNote")}</p>
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
        <Input
          name={`${providerKey}-endpoint`}
          type="url"
          inputMode="url"
          aria-label={t("apiKeys.endpoint")}
          autoComplete="off"
          spellCheck={false}
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
          placeholder={t("apiKeys.endpointPlaceholder")}
          className="text-xs"
        />
      )}

      <div className="flex gap-2">
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
            variant="destructive"
            onClick={handleRemove}
            disabled={isLoading}
            className="text-xs"
          >
            {t("apiKeys.remove")}
          </Button>
        )}
      </div>
    </div>
  );
}
