import type { Accent, Density, TextSize, Theme } from '../../api/types';
import { useAppStore } from '../../state/store';
import { Choice, Row, Section, Swatches } from './controls';
import styles from './Settings.module.css';

const ACCENTS: { value: Accent; color: string; label: string }[] = [
  { value: 'blue', color: '#2F7BFF', label: 'Bleu' },
  { value: 'violet', color: '#8B5CF6', label: 'Violet' },
  { value: 'green', color: '#10B981', label: 'Vert' },
  { value: 'orange', color: '#F59E0B', label: 'Orange' },
  { value: 'pink', color: '#EC4899', label: 'Rose' },
  { value: 'teal', color: '#14B8A6', label: 'Turquoise' },
];

export function AppearanceSection({ onError }: { onError(message: string): void }) {
  const settings = useAppStore((s) => s.settings);
  const setSetting = useAppStore((s) => s.setSetting);
  const save = (key: Parameters<typeof setSetting>[0], value: unknown) => void setSetting(key, value).catch((e) => onError(String(e?.message ?? e)));
  return (
    <Section title="Apparence">
      <Row label="Thème" hint="« Automatique » suit le thème de Windows.">
        <Choice<Theme>
          label="Thème"
          value={settings.theme}
          options={[
            { value: 'dark', label: 'Sombre' },
            { value: 'light', label: 'Clair' },
            { value: 'system', label: 'Automatique' },
          ]}
          onChange={(v) => save('theme', v)}
        />
      </Row>
      <Row label="Couleur d'accent" hint="Les pastilles des comptes gardent leur couleur.">
        <Swatches label="Couleur d'accent" value={settings.accent} options={ACCENTS} onChange={(v) => save('accent', v)} />
      </Row>
      <Row label="Taille du texte">
        <Choice<TextSize>
          label="Taille du texte"
          value={settings.textSize}
          options={[
            { value: 'small', label: 'Petite' },
            { value: 'normal', label: 'Normale' },
            { value: 'large', label: 'Grande' },
          ]}
          onChange={(v) => save('text_size', v)}
        />
      </Row>
      <Row label="Densité de la liste">
        <Choice<Density>
          label="Densité de la liste"
          value={settings.density}
          options={[
            { value: 'comfortable', label: 'Confortable' },
            { value: 'compact', label: 'Compacte' },
          ]}
          onChange={(v) => save('density', v)}
        />
      </Row>
      <Row label="Extrait">
        <label className={styles.check}>
          <input type="checkbox" checked={settings.showSnippet} onChange={(e) => save('show_snippet', e.target.checked)} />
          Afficher l'extrait dans la liste
        </label>
      </Row>
    </Section>
  );
}
