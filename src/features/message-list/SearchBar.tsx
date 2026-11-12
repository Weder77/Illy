import { IconSearch } from '../../ui/icons';
import styles from './MessageList.module.css';

/** Recherche : visible mais inactive jusqu'au jalon 3. */
export function SearchBar() {
  return (
    <>
      <div className={styles.search} title="Disponible bientôt">
        <label className={styles.searchField}>
          <IconSearch size={15} />
          <input placeholder="Rechercher…" disabled aria-label="Rechercher" />
        </label>
        <button className={styles.everywhere} disabled>
          Chercher partout
        </button>
      </div>
      <div className={styles.chips} title="Disponible bientôt">
        {['Non lus', 'Pièce jointe', 'Dates'].map((c) => (
          <button key={c} className={styles.chip} disabled>
            {c}
          </button>
        ))}
      </div>
    </>
  );
}
