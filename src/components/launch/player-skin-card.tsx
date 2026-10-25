import { Box, HStack, Switch, Text } from "@chakra-ui/react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import SkinPreview from "@/components/skin-preview";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { TextureType } from "@/enums/account";
import { UtilsService } from "@/services/utils";
import styles from "@/styles/launch-home.module.css";
import { base64ImgSrc } from "@/utils/string";

// Bundled fallback skin, so a player without a skin texture still shows a body
// instead of an empty canvas. It is a local asset, never a third-party CDN.
const FALLBACK_SKIN_SRC = "/images/skins/steve.png";
// Size envelope of the showcase. It no longer fills the action column: the
// whole card, id badge and cape row included, is capped at 210x320 so it stays
// a skin floating above the launch button. .playerSkinCard keeps the same
// max-width, update both together.
const MAX_CARD_WIDTH = 210;
const MAX_CARD_HEIGHT = 320;
// Floors: a narrower column shrinks the preview, but never to nothing.
const MIN_CANVAS_WIDTH = 120;
const MIN_CANVAS_HEIGHT = 140;
// The id pill above the skin and the distance that separates it from the skin.
const BADGE_HEIGHT = 24;
const BADGE_GAP = 10;
// SkinPreview always reserves 40px for the control bar it draws. We hide that
// bar (showControlBar={false}) and use the strip for our single cape row.
const PREVIEW_RESERVED_BAR = 40;
// One right-aligned row under the skin: the only control left on the card.
const CONTROL_BAR_HEIGHT = 28;

// Transparent 3D showcase of the player selected on the launch page: the skin
// floats on the wallpaper with the player id above it, and nothing is rendered
// while no player is selected.
const PlayerSkinCard = () => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const { selectedPlayer } = useGlobalData();
  const primaryColor = config.appearance.theme.primaryColor;
  const [isCapeVisible, setIsCapeVisible] = useState(true);
  const [fontFamily, setFontFamily] = useState<string | null>(null);
  const [cardWidth, setCardWidth] = useState(0);
  const cardRef = useRef<HTMLDivElement>(null);

  // Minecraft is not a bundled font: if the machine already has one installed
  // (shipped by the launcher or by the game itself), borrow it for the id pill.
  // The lookup runs after mount, never on render, and any failure simply leaves
  // the pill on the inherited font.
  useEffect(() => {
    let active = true;

    UtilsService.retrieveFontList()
      .then((response) => {
        if (!active || response.status !== "success") return;
        const minecraftFont = response.data.find((font) =>
          /minecraft/i.test(font)
        );
        if (minecraftFont) setFontFamily(`"${minecraftFont}"`);
      })
      .catch(() => {
        // A missing font list must not break the preview.
      });

    return () => {
      active = false;
    };
  }, []);

  // The column caps the card, but a narrower window must still shrink the
  // canvas, so its box is measured instead of assumed. The effect re-runs on a
  // player change because the card only exists once a player is selected.
  useEffect(() => {
    const card = cardRef.current;
    if (!card) return;

    const syncCardWidth = () => {
      setCardWidth(Math.floor(card.getBoundingClientRect().width));
    };

    syncCardWidth();
    const observer = new ResizeObserver(syncCardWidth);
    observer.observe(card);
    return () => observer.disconnect();
  }, [selectedPlayer]);

  if (!selectedPlayer) return null;

  const skin = selectedPlayer.textures.find(
    (texture) => texture.textureType === TextureType.Skin
  );
  const cape = selectedPlayer.textures.find(
    (texture) => texture.textureType === TextureType.Cape
  );

  const canvasWidth = Math.min(
    Math.max(cardWidth || MAX_CARD_WIDTH, MIN_CANVAS_WIDTH),
    MAX_CARD_WIDTH
  );
  // Keep the 5:6 silhouette of the default skin, clamped by the height the card
  // may still occupy under the badge.
  const maxCanvasHeight =
    MAX_CARD_HEIGHT - BADGE_HEIGHT - BADGE_GAP - PREVIEW_RESERVED_BAR;
  const canvasHeight = Math.min(
    Math.max(Math.round((canvasWidth * 6) / 5), MIN_CANVAS_HEIGHT),
    maxCanvasHeight
  );
  const previewHeight = canvasHeight + PREVIEW_RESERVED_BAR;

  return (
    <Box
      ref={cardRef}
      data-launch="player-skin"
      className={styles.playerSkinCard}
    >
      <Box
        data-launch="player-skin-id"
        className={styles.playerSkinBadge}
        title={selectedPlayer.name}
        style={fontFamily ? { fontFamily } : undefined}
      >
        {selectedPlayer.name}
      </Box>
      <Box
        className={styles.playerSkinStage}
        style={{
          width: `${canvasWidth}px`,
          height: `${previewHeight}px`,
        }}
      >
        <SkinPreview
          skinSrc={skin ? base64ImgSrc(skin.image) : FALLBACK_SKIN_SRC}
          capeSrc={cape ? base64ImgSrc(cape.image) : undefined}
          skinModel={skin?.model}
          width={canvasWidth}
          height={previewHeight}
          // No plate behind the skin: "none" is SkinPreview's transparent
          // backgroundType, it clears the viewer background so the wallpaper
          // shows through the canvas.
          canvasBg="none"
          showControlBar={false}
          isCapeVisible={isCapeVisible}
          onCapeVisibilityChange={setIsCapeVisible}
        />
        <HStack
          className={styles.playerSkinControls}
          style={{ height: `${CONTROL_BAR_HEIGHT}px` }}
        >
          <Text fontSize="xs" className="secondary-text" whiteSpace="nowrap">
            {t("SkinPreview.cape")}
          </Text>
          <Switch
            size="sm"
            colorScheme={primaryColor}
            isChecked={isCapeVisible}
            onChange={(event) => setIsCapeVisible(event.target.checked)}
            aria-label={t("SkinPreview.cape")}
          />
        </HStack>
      </Box>
    </Box>
  );
};

export default PlayerSkinCard;
