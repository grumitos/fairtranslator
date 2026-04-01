import i18n from "i18next";
import { initReactI18next } from "react-i18next";

// ES locales
import esCommon from "@/locales/es/common.json";
import esEditor from "@/locales/es/editor.json";
import esSettings from "@/locales/es/settings.json";
import esGlossary from "@/locales/es/glossary.json";

// EN locales
import enCommon from "@/locales/en/common.json";
import enEditor from "@/locales/en/editor.json";
import enSettings from "@/locales/en/settings.json";
import enGlossary from "@/locales/en/glossary.json";

const resources = {
  es: {
    common: esCommon,
    editor: esEditor,
    settings: esSettings,
    glossary: esGlossary,
  },
  en: {
    common: enCommon,
    editor: enEditor,
    settings: enSettings,
    glossary: enGlossary,
  },
};

const SETTINGS_STORAGE_KEY = "fairtranslator-settings";

function getInitialLanguage(): "es" | "en" {
  if (typeof window === "undefined") {
    return "es";
  }

  try {
    const raw = window.localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return "es";
    }

    const parsed = JSON.parse(raw) as {
      state?: { language?: "es" | "en" };
    };
    const language = parsed.state?.language;
    return language === "en" ? "en" : "es";
  } catch {
    return "es";
  }
}

i18n.use(initReactI18next).init({
  resources,
  lng: getInitialLanguage(),
  fallbackLng: "en",
  defaultNS: "common",
  ns: ["common", "editor", "settings", "glossary"],
  interpolation: {
    escapeValue: false, // React already escapes
  },
});

export default i18n;
