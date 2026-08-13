import de from "./locales/de";
import en from "./locales/en";
import es from "./locales/es";
import fr from "./locales/fr";
import it from "./locales/it";
import ja from "./locales/ja";
import ko from "./locales/ko";
import ru from "./locales/ru";
import zhHant from "./locales/zhHant";

/** The languages the window can render in. */
export type Locale =
  | "en"
  | "zh-Hans"
  | "zh-Hant"
  | "de"
  | "fr"
  | "es"
  | "it"
  | "ru"
  | "ja"
  | "ko";

/**
 * Translations keyed by the Chinese source text.
 *
 * The source string is the key, the way gettext works, for two reasons: 206
 * invented keys would be 206 chances to mistype one, and a key with no entry
 * falls back to readable text rather than to a blank or an identifier. The key
 * being Chinese is an implementation detail — English is what the window opens
 * in.
 */
export type Dictionary = Readonly<Record<string, string>>;

const DICTIONARIES: Readonly<Record<Locale, Dictionary>> = {
  en,
  // Simplified Chinese is the language the source strings are written in, so it
  // needs no dictionary: every lookup falls through to the key itself.
  "zh-Hans": {},
  "zh-Hant": zhHant,
  de,
  fr,
  es,
  it,
  ru,
  ja,
  ko,
};

/** What the window opens in before the user has chosen anything. */
export const DEFAULT_LOCALE: Locale = "en";

/** Every language, in the order the picker offers them. */
export const LOCALES: readonly { id: Locale; label: string }[] = [
  { id: "en", label: "English" },
  { id: "zh-Hans", label: "简体中文" },
  { id: "zh-Hant", label: "繁體中文" },
  { id: "de", label: "Deutsch" },
  { id: "fr", label: "Français" },
  { id: "es", label: "Español" },
  { id: "it", label: "Italiano" },
  { id: "ru", label: "Русский" },
  { id: "ja", label: "日本語" },
  { id: "ko", label: "한국어" },
];

/**
 * Looks up `text` in `locale`, falling back to the source string.
 *
 * The fallback is deliberate: an untranslated label reads as Chinese in an
 * otherwise English window, which tells the user — and whoever reports it —
 * that a translation is missing, rather than hiding it behind an empty span. An
 * unknown locale falls back the same way: a stored value this build does not
 * know must not take the whole window down over one label.
 */
export function translate(locale: Locale, text: string): string {
  return DICTIONARIES[locale]?.[text] ?? text;
}

/** Every source string that has no translation in `locale`. */
export function untranslated(
  locale: Locale,
  texts: readonly string[],
): string[] {
  const dictionary = DICTIONARIES[locale] ?? {};
  return texts.filter((text) => dictionary[text] === undefined);
}

/** The keys a language covers, for the test that keeps them in step. */
export function translatedKeys(locale: Locale): string[] {
  return Object.keys(DICTIONARIES[locale] ?? {});
}
