import {
  Box,
  Button,
  Center,
  Flex,
  HStack,
  IconButton,
  IconButtonProps,
  Popover,
  PopoverBody,
  PopoverContent,
  PopoverTrigger,
  Portal,
  Text,
  Tooltip,
  VStack,
  useColorModeValue,
  useDisclosure,
  useToken,
} from "@chakra-ui/react";
import { useRouter } from "next/router";
import { cloneElement, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuArrowLeftRight, LuSettings } from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { CommonIconButton } from "@/components/common/common-icon-button";
import { CompactButtonGroup } from "@/components/common/compact-button-group";
import InstancesView from "@/components/instances-view";
import PlayerAvatar from "@/components/player-avatar";
import LiquidGlassEffect from "@/components/special/liquid-glass-effect";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { useSharedModals } from "@/contexts/shared-modal";
import { PlayerType } from "@/enums/account";
import styles from "@/styles/launch.module.css";

interface CustomButtonProps extends Omit<IconButtonProps, "onClick"> {
  tooltip: string;
  onClick: () => void;
  popoverContent: React.ReactElement;
}

const ButtonWithPopover: React.FC<CustomButtonProps> = ({
  tooltip,
  popoverContent,
  onClick,
  ...props
}) => {
  const { config } = useLauncherConfig();
  const quickSwitch = config.general.functionality.launchPageQuickSwitch;
  const { isOpen, onToggle, onClose } = useDisclosure();
  const [tooltipDisabled, setTooltipDisabled] = useState(false);

  const handleClose = () => {
    setTooltipDisabled(true);
    onClose();
    setTimeout(() => setTooltipDisabled(false), 200);
  };

  return (
    <Popover
      isOpen={isOpen}
      onClose={handleClose}
      placement="top-end"
      gutter={12}
    >
      <Tooltip label={tooltip} placement="top-end" isDisabled={tooltipDisabled}>
        <Box lineHeight={0}>
          <PopoverTrigger>
            <IconButton
              size="xs"
              icon={<LuArrowLeftRight />}
              {...props}
              onClick={() => {
                quickSwitch ? onToggle() : onClick();
              }}
            />
          </PopoverTrigger>
        </Box>
      </Tooltip>

      <Portal>
        <PopoverContent maxH="3xs" overflow="auto">
          <PopoverBody p={0}>
            {cloneElement(popoverContent, {
              onSelectCallback: () => setTimeout(handleClose, 100),
            })}
          </PopoverBody>
        </PopoverContent>
      </Portal>
    </Popover>
  );
};

const HomeButtonGroup = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { openSharedModal } = useSharedModals();
  const { config } = useLauncherConfig();
  const useLiquidGlass = config.appearance.theme.useLiquidGlassDesign;
  const primaryColor = config.appearance.theme.primaryColor;
  const [launchRingColor] = useToken("colors", [`${primaryColor}.500`]);
  const subtitleColor = useColorModeValue("gray.600", "gray.400");
  const compactButtonGroupColorScheme = useColorModeValue("blackAlpha", "gray");
  const { selectedPlayer, selectedInstance, getPlayerList, getInstanceList } =
    useGlobalData();

  const playerList = getPlayerList() || [];
  const instanceList = getInstanceList() || [];
  const hasPlayers = playerList.length > 0;
  const hasInstances = instanceList.length > 0;

  const [isAddZoneHovered, setIsAddZoneHovered] = useState(false);
  const [isAddZoneFocused, setIsAddZoneFocused] = useState(false);
  const isAddZoneActive = isAddZoneHovered || isAddZoneFocused;

  const [isPlayerZoneHovered, setIsPlayerZoneHovered] = useState(false);
  const [isPlayerZoneFocused, setIsPlayerZoneFocused] = useState(false);
  const isPlayerZoneActive = isPlayerZoneHovered || isPlayerZoneFocused;

  // Mirror the launch button: only the right quarter reveals the switch target.
  const handlePlayerPointerMove = (
    event: React.PointerEvent<HTMLDivElement>
  ) => {
    if (event.pointerType === "touch") return;
    const rect = event.currentTarget.getBoundingClientRect();
    setIsPlayerZoneHovered(event.clientX - rect.left >= rect.width * 0.75);
  };

  // Track the pointer against the whole button so the right quarter stays
  // active while it travels over the floating instance controls above it.
  const handleLaunchPointerMove = (
    event: React.PointerEvent<HTMLDivElement>
  ) => {
    if (event.pointerType === "touch") return;
    const rect = event.currentTarget.getBoundingClientRect();
    setIsAddZoneHovered(event.clientX - rect.left >= rect.width * 0.75);
  };

  return (
    <Flex wrap="wrap" justify="flex-end" align="flex-end" gap={4}>
      <AdvancedCard
        className={styles["selected-user-card"]}
        position="relative"
        data-liquid={useLiquidGlass}
        data-player-active={isPlayerZoneActive}
        onPointerMove={handlePlayerPointerMove}
        onPointerLeave={() => setIsPlayerZoneHovered(false)}
      >
        <Box h="100%" p={3}>
          {selectedPlayer ? (
            <Flex h="100%" w="100%" align="center">
              <HStack
                spacing={2.5}
                h="100%"
                minW={0}
                className={styles["player-card-group"]}
              >
                <PlayerAvatar
                  boxSize="36px"
                  objectFit="cover"
                  avatar={selectedPlayer.avatar}
                />
                <VStack spacing={0} align="left" mt={-2} minW={0}>
                  <Text
                    fontSize="sm"
                    fontWeight="bold"
                    maxW="100%"
                    mt={2}
                    isTruncated
                  >
                    {selectedPlayer.name}
                  </Text>
                  <Text fontSize="xs" color={subtitleColor} lineHeight={4}>
                    {t(
                      `Enums.playerTypes.${selectedPlayer.playerType === PlayerType.ThirdParty ? "3rdpartyShort" : selectedPlayer.playerType}`
                    )}
                  </Text>
                  <Text fontSize="xs" color={subtitleColor} lineHeight={4}>
                    {selectedPlayer.playerType === PlayerType.ThirdParty &&
                      selectedPlayer.authServer?.name}
                  </Text>
                </VStack>
              </HStack>
            </Flex>
          ) : (
            <Center w="100%" h="100%" className={styles["player-card-empty"]}>
              <Text fontSize="sm" className="secondary-text">
                {t("LaunchPage.Text.noSelectedPlayer")}
              </Text>
            </Center>
          )}
        </Box>

        <button
          type="button"
          aria-label={t(
            `LaunchPage.SwitchButton.tooltip.${hasPlayers ? "switchPlayer" : "addPlayer"}`
          )}
          title={t(
            `LaunchPage.SwitchButton.tooltip.${hasPlayers ? "switchPlayer" : "addPlayer"}`
          )}
          className={`${styles["card-quarter-zone"]} ${styles["player-card-zone"]}`}
          data-active={isPlayerZoneActive}
          onFocus={() => setIsPlayerZoneFocused(true)}
          onBlur={() => setIsPlayerZoneFocused(false)}
          onClick={() =>
            router.push(hasPlayers ? "/accounts" : "/accounts?add=true")
          }
        >
          <span className={styles["player-card-surface"]} aria-hidden="true" />
          <span className={styles["player-card-icon"]} aria-hidden="true">
            <svg viewBox="0 0 28 28" focusable="false">
              <circle cx="14" cy="14" r="11.25" />
              <path d="M14 8.5v11M8.5 14h11" />
            </svg>
          </span>
        </button>

        <span className={styles["player-card-divider"]} aria-hidden="true" />
      </AdvancedCard>

      <Flex direction="column" align="flex-end">
        {hasInstances && (
          <HStack spacing={1} justify="flex-end" mb={2} zIndex={5}>
            <CompactButtonGroup
              colorScheme={compactButtonGroupColorScheme}
              size="xs"
            >
              {selectedInstance && (
                <CommonIconButton
                  icon={LuSettings}
                  label={t("LaunchPage.button.instanceSettings")}
                  tooltipPlacement="top"
                  onClick={() =>
                    router.push({
                      pathname: "/instances/details/[id]/settings",
                      query: { id: selectedInstance.id },
                    })
                  }
                />
              )}

              <ButtonWithPopover
                tooltip={t("LaunchPage.SwitchButton.tooltip.switchInstance")}
                aria-label="instance"
                popoverContent={
                  <InstancesView
                    instances={instanceList}
                    selectedInstance={selectedInstance}
                    viewType="list"
                    withMenu={false}
                  />
                }
                onClick={() => router.push("/instances/list")}
              />
            </CompactButtonGroup>
          </HStack>
        )}

        <Box
          position="relative"
          className={styles["launch-button-shell"]}
          style={
            { "--launch-ring-color": launchRingColor } as React.CSSProperties
          }
          data-liquid={useLiquidGlass}
          data-add-active={isAddZoneActive}
          onPointerMove={handleLaunchPointerMove}
          onPointerLeave={() => setIsAddZoneHovered(false)}
        >
          <Button
            id="main-launch-button"
            data-liquid={useLiquidGlass}
            colorScheme="blackAlpha"
            className={styles["launch-button"]}
            onClick={() => {
              if (selectedInstance) {
                openSharedModal("launch", {
                  instanceId: selectedInstance.id,
                });
              }
            }}
          >
            {useLiquidGlass && <LiquidGlassEffect isDark />}
            <VStack
              spacing={1.5}
              w="100%"
              color="white"
              position="relative"
              zIndex={useLiquidGlass ? 3 : undefined}
              className={styles["launch-button-label"]}
            >
              <Text fontSize="lg" fontWeight="bold">
                {t("LaunchPage.button.launch")}
              </Text>
              <Text fontSize="sm" className="ellipsis-text">
                {selectedInstance
                  ? selectedInstance.name
                  : t("LaunchPage.Text.noSelectedGame")}
              </Text>
            </VStack>
          </Button>

          {/* The old standalone add-instance icon button was removed from the
            compact control group; this quarter is now its whole click target. */}
          <button
            type="button"
            aria-label={t("LaunchPage.SwitchButton.tooltip.addInstance")}
            title={t("LaunchPage.SwitchButton.tooltip.addInstance")}
            className={`${styles["card-quarter-zone"]} ${styles["launch-add-zone"]}`}
            data-active={isAddZoneActive}
            onFocus={() => setIsAddZoneFocused(true)}
            onBlur={() => setIsAddZoneFocused(false)}
            onClick={() => router.push("/instances/add-import")}
          >
            <span className={styles["launch-add-surface"]} aria-hidden="true" />
            <span className={styles["launch-add-icon"]} aria-hidden="true">
              <svg viewBox="0 0 28 28" focusable="false">
                <circle cx="14" cy="14" r="11.25" />
                <path d="M14 8.5v11M8.5 14h11" />
              </svg>
            </span>
          </button>

          <span className={styles["launch-divider"]} aria-hidden="true" />
        </Box>
      </Flex>
    </Flex>
  );
};

export default HomeButtonGroup;
