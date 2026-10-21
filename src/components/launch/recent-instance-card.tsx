import {
  Center,
  HStack,
  Icon,
  Image,
  Tag,
  Text,
  VStack,
} from "@chakra-ui/react";
import { useRouter } from "next/router";
import { useTranslation } from "react-i18next";
import { LuArrowUpRight, LuClock3, LuPlay, LuStar } from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { CommonIconButton } from "@/components/common/common-icon-button";
import { Section } from "@/components/common/section";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { useSharedModals } from "@/contexts/shared-modal";
import { ModLoaderType } from "@/enums/instance";
import { InstanceSummary } from "@/models/instance/misc";
import { getInstanceIconSrc, parseModLoaderVersion } from "@/utils/instance";

const formatPlaytime = (seconds: number) => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
};

const getLoaderLabel = (instance: InstanceSummary) => {
  const loader = instance.modLoader;
  if (!loader || loader.loaderType === ModLoaderType.Unknown) return "";
  return [loader.loaderType, parseModLoaderVersion(loader.version || "")]
    .filter(Boolean)
    .join(" ");
};

// The most relevant instance: the current selection, falling back to the first
// (starred-first) entry of the list already loaded by the global data context.
const RecentInstanceCard = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { config } = useLauncherConfig();
  const { selectedInstance, getInstanceList } = useGlobalData();
  const { openSharedModal } = useSharedModals();
  const primaryColor = config.appearance.theme.primaryColor;

  const instance = selectedInstance || (getInstanceList() || [])[0];
  if (!instance) return null;

  const loaderLabel = getLoaderLabel(instance);

  const handleLaunch = () => {
    openSharedModal("launch", { instanceId: instance.id });
  };

  const handleOpenDetails = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    router.push({
      pathname: "/instances/details/[id]/overview",
      query: { id: instance.id },
    });
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    handleLaunch();
  };

  return (
    <Section
      data-launch="recent-instance"
      title={t("LaunchPage.recentInstance.title")}
      headExtra={
        <CommonIconButton
          icon="more"
          label={t("LaunchPage.recentInstance.viewAll")}
          size="xs"
          fontSize="sm"
          h={21}
          onClick={() => router.push("/instances/list")}
        />
      }
    >
      <AdvancedCard
        p={{ base: 3, xl: 3.5 }}
        cursor="pointer"
        role="button"
        tabIndex={0}
        aria-label={t("LaunchPage.recentInstance.launch")}
        transition="transform 0.18s ease, box-shadow 0.18s ease"
        _hover={{ transform: "translateY(-2px)", boxShadow: "lg" }}
        _focusVisible={{
          outline: "2px solid",
          outlineColor: `${primaryColor}.500`,
          outlineOffset: "2px",
        }}
        onClick={handleLaunch}
        onKeyDown={handleKeyDown}
      >
        <HStack spacing={{ base: 3, xl: 4 }} align="center" minW={0}>
          <Image
            src={getInstanceIconSrc(instance.iconSrc, instance.versionPath)}
            fallbackSrc="/images/icons/JEIcon_Release.png"
            alt={instance.name}
            boxSize={{ base: "48px", xl: "56px" }}
            flexShrink={0}
            borderRadius="lg"
            objectFit="cover"
          />
          <VStack align="start" spacing={1.5} flex={1} minW={0}>
            <HStack spacing={1.5} maxW="100%" minW={0}>
              <Text
                fontSize={{ base: "sm", xl: "md" }}
                fontWeight="bold"
                isTruncated
              >
                {instance.name}
              </Text>
              {instance.starred && (
                <Icon
                  as={LuStar}
                  boxSize={3.5}
                  color={`${primaryColor}.400`}
                  fill="currentColor"
                  flexShrink={0}
                />
              )}
            </HStack>
            <HStack spacing={1.5} wrap="wrap" minW={0}>
              {loaderLabel && (
                <Tag
                  size="sm"
                  colorScheme={primaryColor}
                  variant="subtle"
                  flexShrink={0}
                >
                  {loaderLabel}
                </Tag>
              )}
              <Tag size="sm" variant="outline" flexShrink={0}>
                {instance.version}
              </Tag>
              <HStack spacing={1} className="secondary-text" fontSize="xs">
                <Icon as={LuClock3} boxSize={3} flexShrink={0} />
                <Text noOfLines={1}>
                  {t("LaunchPage.recentInstance.playtime", {
                    time: formatPlaytime(instance.playTime),
                  })}
                </Text>
              </HStack>
            </HStack>
          </VStack>
          <HStack spacing={{ base: 0.5, xl: 1.5 }} flexShrink={0}>
            <CommonIconButton
              icon={LuArrowUpRight}
              label={t("LaunchPage.recentInstance.details")}
              size="xs"
              fontSize="sm"
              onClick={handleOpenDetails}
            />
            <Center
              boxSize={{ base: "2.25rem", xl: "2.5rem" }}
              borderRadius="full"
              bg={`${primaryColor}.500`}
              color="white"
              boxShadow="sm"
            >
              <Icon as={LuPlay} boxSize={3.5} />
            </Center>
          </HStack>
        </HStack>
      </AdvancedCard>
    </Section>
  );
};

export default RecentInstanceCard;
