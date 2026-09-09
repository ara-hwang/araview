import i18n from "i18next"
import { initReactI18next } from "react-i18next"
import en from "./locales/en.json"
import ko from "./locales/ko.json"

export type AppLanguage = "ko" | "en"

export const SUPPORTED_LANGUAGES: AppLanguage[] = ["ko", "en"]

export function detectSystemLanguage(): AppLanguage {
  if (typeof navigator !== "undefined") {
    const lang = navigator.language?.toLowerCase() ?? ""
    if (lang.startsWith("ko")) return "ko"
  }
  return "en"
}

export function normalizeLanguage(value: unknown): AppLanguage | null {
  return value === "ko" || value === "en" ? value : null
}

export async function initI18n(lng: AppLanguage): Promise<void> {
  if (i18n.isInitialized) {
    if (i18n.language !== lng) await i18n.changeLanguage(lng)
    return
  }
  await i18n.use(initReactI18next).init({
    resources: {
      ko: { translation: ko },
      en: { translation: en }
    },
    lng,
    fallbackLng: "ko",
    supportedLngs: ["ko", "en"],
    interpolation: { escapeValue: false },
    returnEmptyString: false
  })
}

export async function setI18nLanguage(lng: AppLanguage): Promise<void> {
  if (!i18n.isInitialized) {
    await initI18n(lng)
    return
  }
  if (i18n.language !== lng) await i18n.changeLanguage(lng)
}

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation"
    resources: {
      translation: typeof ko
    }
  }
}

export default i18n
