import styles from "@/styles/background-preset.module.css";

interface BackgroundPresetProps {
  presetKey: string;
  colorMode: "light" | "dark";
  fixed?: boolean;
}

// Purely decorative, CSS-only animated backgrounds that are not image files.
const BackgroundPreset = ({
  presetKey,
  colorMode,
  fixed = false,
}: BackgroundPresetProps) => {
  if (presetKey !== "Meridian") return null;

  return (
    <div
      className={`${styles.layer} ${fixed ? styles.fixed : ""}`}
      data-theme={colorMode}
      aria-hidden="true"
    >
      <div className={styles.canvas}>
        <div className={styles.base} />
        <div className={`${styles.aurora} ${styles["aurora-a"]}`} />
        <div className={`${styles.aurora} ${styles["aurora-b"]}`} />
        <div className={`${styles.aurora} ${styles["aurora-c"]}`} />
        <div className={styles.grille}>
          <div className={styles.grid} />
        </div>
        <div className={styles.vignette} />
      </div>
    </div>
  );
};

export default BackgroundPreset;
