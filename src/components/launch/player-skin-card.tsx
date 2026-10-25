import { Box, HStack, Switch, Text, Tooltip } from "@chakra-ui/react";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuSave } from "react-icons/lu";
import { CommonIconButton } from "@/components/common/common-icon-button";
import ManageSkinModal from "@/components/modals/manage-skin-modal";
import SkinPreview from "@/components/skin-preview";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { useToast } from "@/contexts/toast";
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
// bar (showControlBar={false}) and use the strip for the card's own controls.
const PREVIEW_RESERVED_BAR = 40;
// One right-aligned row under the skin: save the current skin, toggle the cape.
const CONTROL_BAR_HEIGHT = 28;

// Transparent 3D showcase of the player selected on the launch page: the skin
// floats on the wallpaper with the player id above it, and nothing is rendered
// while no player is selected.
const PlayerSkinCard = () => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const { selectedPlayer, getPlayerList } = useGlobalData();
  const toast = useToast();
  const primaryColor = config.appearance.theme.primaryColor;
  const [isCapeVisible, setIsCapeVisible] = useState(true);
  const [isSkinManagerOpen, setIsSkinManagerOpen] = useState(false);
  const [fontFamily, setFontFamily] = useState<string | null>(null);
  const [cardWidth, setCardWidth] = useState(0);
  const cardRef = useRef<HTMLDivElement>(null);
  // Dragging on the model rotates it, so the click that ends a rotation drag must
  // not pop the texture manager: remember where the press started.
  const pointerDownRef = useRef<{ x: number; y: number } | null>(null);

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

  // Same entry point as PCL: the skin itself is clickable, hints 点击更换皮肤 on
  // hover and opens the texture manager for the signed-in player. The modal is
  // mounted here with card-local state, so it never shares the accounts page's.
  const hint = t("LaunchPage.playerSkin.changeSkin");

  const handleCardPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    pointerDownRef.current = { x: event.clientX, y: event.clientY };
  };

  const handleOpenSkinManager = () => {
    pointerDownRef.current = null;
    setIsSkinManagerOpen(true);
  };

  const handleCardClick = (event: React.MouseEvent<HTMLDivElement>) => {
    const start = pointerDownRef.current;
    pointerDownRef.current = null;
    if (
      start &&
      Math.hypot(event.clientX - start.x, event.clientY - start.y) > 4
    ) {
      return;
    }
    handleOpenSkinManager();
  };

  const handleSkinManagerClose = () => {
    setIsSkinManagerOpen(false);
    // The player list is the shared source of truth selectedPlayer is derived
    // from, and re-reading it is a local list copy: doing it on every close
    // means a change that landed in the manager shows up on the model and the
    // avatar even if the modal dismissed before its own refresh resolved.
    getPlayerList(true);
  };

  // The cape visibility switch is a cape switch: it gets its own handler and
  // must never open the manager, so the card click stays separate.
  const handleCapeVisibilityToggle = (checked: boolean) => {
    setIsCapeVisible(checked);
  };

  // Enter and Space on the card itself open the manager. Keys pressed on the
  // controls inside the card stay with those controls (target is not the card).
  const handleCardKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    handleOpenSkinManager();
  };

  // PCL keeps 保存皮肤 on the skin widget. SkinPreview only draws its save icon
  // inside the control bar this card hides, so the card offers the same action
  // in its own row, next to the cape switch.
  const handleSaveSkin = async () => {
    if (!skin?.image) return;

    const savePath = await save({
      defaultPath: "skin.png",
      filters: [
        {
          name: t("General.dialog.filterName.image"),
          extensions: ["png"],
        },
      ],
    });
    if (!savePath) return;

    const response = await UtilsService.writeFile(
      savePath,
      skin.image,
      "base64"
    );
    toast({
      title: t(`SkinPreview.toast.${response.status}`),
      description: response.status === "error" ? response.details : undefined,
      status: response.status,
    });
  };

  return (
    <>
      <Tooltip label={hint} placement="top" hasArrow>
        <Box
          ref={cardRef}
          data-launch="player-skin"
          className={styles.playerSkinCard}
          role="button"
          tabIndex={0}
          aria-label={hint}
          aria-haspopup="dialog"
          cursor="pointer"
          transition="transform 0.18s ease, filter 0.18s ease"
          // PCL lifts a soft shadow under the model on hover. drop-shadow follows
          // the transparent canvas, so it traces the skin instead of the box.
          _hover={{
            transform: "translateY(-2px)",
            filter: "drop-shadow(0 6px 10px rgba(0, 0, 0, 0.35))",
          }}
          _focusVisible={{
            outline: "2px solid",
            outlineColor: `${primaryColor}.500`,
            outlineOffset: "2px",
          }}
          onPointerDown={handleCardPointerDown}
          onClick={handleCardClick}
          onKeyDown={handleCardKeyDown}
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
              // The bar does not fit the card, but the save action it carries
              // stays reachable: the row below mirrors it with the same label.
              showSaveButton
              isCapeVisible={isCapeVisible}
              onCapeVisibilityChange={setIsCapeVisible}
            />
            <HStack
              className={styles.playerSkinControls}
              style={{ height: `${CONTROL_BAR_HEIGHT}px` }}
              // Clicks on the row's controls must not also open the manager.
              onClick={(event) => event.stopPropagation()}
            >
              {skin?.image && (
                <CommonIconButton
                  icon={LuSave}
                  size="xs"
                  label={t("SkinPreview.button.saveToLocal")}
                  tooltipPlacement="top"
                  onClick={handleSaveSkin}
                />
              )}
              <Text
                fontSize="xs"
                className="secondary-text"
                whiteSpace="nowrap"
              >
                {t("SkinPreview.cape")}
              </Text>
              <Switch
                size="sm"
                colorScheme={primaryColor}
                isChecked={isCapeVisible}
                onChange={(event) =>
                  handleCapeVisibilityToggle(event.target.checked)
                }
                aria-label={t("SkinPreview.cape")}
              />
            </HStack>
          </Box>
        </Box>
      </Tooltip>
      <ManageSkinModal
        isOpen={isSkinManagerOpen}
        onClose={handleSkinManagerClose}
        player={selectedPlayer}
      />
    </>
  );
};

export default PlayerSkinCard;
