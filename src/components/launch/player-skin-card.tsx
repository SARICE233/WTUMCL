import { Box } from "@chakra-ui/react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import AdvancedCard from "@/components/common/advanced-card";
import { Section } from "@/components/common/section";
import SkinPreview from "@/components/skin-preview";
import { useGlobalData } from "@/contexts/global-data";
import { TextureType } from "@/enums/account";
import styles from "@/styles/launch-home.module.css";
import { base64ImgSrc } from "@/utils/string";

// Bundled fallback skin, so a player without a skin texture still shows a body
// instead of an empty canvas. It is a local asset, never a third-party CDN.
const FALLBACK_SKIN_SRC = "/images/skins/steve.png";
// skinview3d renders into a fixed-size canvas while the card is fluid, so the
// card measures the body it gets and fits a 5:6 canvas into it; the control bar
// adds its own 40px on top, exactly like the other preview call sites.
const CANVAS_HEIGHT_RATIO = 1.2;
const CONTROL_BAR_HEIGHT = 40;
const MIN_CANVAS_WIDTH = 120;
const MIN_CANVAS_HEIGHT = 120;
// Used before the first measurement and in the stacked layout, where the card
// has no definite height from its column and would otherwise collapse.
const DEFAULT_BODY_WIDTH = 14.5 * 16 - 20;
const DEFAULT_BODY_HEIGHT = 300;

// Live 3D preview of the player selected on the launch page: it shows the skin
// and cape textures the account already stores and hides itself while no player
// is selected.
const PlayerSkinCard = () => {
  const { t } = useTranslation();
  const { selectedPlayer } = useGlobalData();
  const [isCapeVisible, setIsCapeVisible] = useState(true);
  const [bodySize, setBodySize] = useState({ width: 0, height: 0 });
  const bodyRef = useRef<HTMLDivElement>(null);

  // The card takes the free column space, so a short window shrinks the preview
  // instead of pushing the launch controls out of view. The effect re-runs on a
  // player change because the body only exists once a player is selected.
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;

    const syncBodySize = () => {
      const rect = body.getBoundingClientRect();
      setBodySize({
        width: Math.floor(rect.width),
        height: Math.floor(rect.height),
      });
    };

    syncBodySize();
    const observer = new ResizeObserver(syncBodySize);
    observer.observe(body);
    return () => observer.disconnect();
  }, [selectedPlayer]);

  if (!selectedPlayer) return null;

  const skin = selectedPlayer.textures.find(
    (texture) => texture.textureType === TextureType.Skin
  );
  const cape = selectedPlayer.textures.find(
    (texture) => texture.textureType === TextureType.Cape
  );

  const bodyWidth = bodySize.width || DEFAULT_BODY_WIDTH;
  const bodyHeight = bodySize.height || DEFAULT_BODY_HEIGHT;
  // Fit a 5:6 canvas into the body like object-fit: contain, with a floor, so it
  // never overflows the card and never collapses either.
  const canvasHeight = Math.max(
    MIN_CANVAS_HEIGHT,
    Math.min(
      Math.round(bodyWidth * CANVAS_HEIGHT_RATIO),
      bodyHeight - CONTROL_BAR_HEIGHT
    )
  );
  const canvasWidth = Math.max(
    MIN_CANVAS_WIDTH,
    Math.min(bodyWidth, Math.round(canvasHeight / CANVAS_HEIGHT_RATIO))
  );
  const height = canvasHeight + CONTROL_BAR_HEIGHT;

  return (
    <Section
      data-launch="player-skin"
      className={styles.playerSkinCard}
      title={t("LaunchPage.playerSkin.title")}
      description={selectedPlayer.name}
    >
      <AdvancedCard p={2.5} flex="1" minH={0}>
        <Box
          ref={bodyRef}
          h="100%"
          minH={0}
          display="flex"
          justifyContent="center"
          alignItems="center"
          overflow="hidden"
        >
          <SkinPreview
            skinSrc={skin ? base64ImgSrc(skin.image) : FALLBACK_SKIN_SRC}
            capeSrc={cape ? base64ImgSrc(cape.image) : undefined}
            skinModel={skin?.model}
            width={canvasWidth}
            height={height}
            isCapeVisible={isCapeVisible}
            onCapeVisibilityChange={setIsCapeVisible}
          />
        </Box>
      </AdvancedCard>
    </Section>
  );
};

export default PlayerSkinCard;
