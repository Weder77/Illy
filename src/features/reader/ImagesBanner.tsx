import { IconImage } from '../../ui/icons';
import styles from './Reader.module.css';

export function ImagesBanner(props: { onShow: () => void; onAlways: () => void }) {
  return (
    <div className={styles.banner} role="status">
      <IconImage size={16} />
      <span className={styles.bannerText}>Images bloquées pour protéger ta vie privée</span>
      <button className={styles.bannerBtn} onClick={props.onShow}>
        Afficher les images
      </button>
      <span className={styles.bannerSep}>·</span>
      <button className={styles.bannerBtn} onClick={props.onAlways}>
        Toujours pour cet expéditeur
      </button>
    </div>
  );
}
