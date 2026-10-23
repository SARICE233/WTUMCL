import { HStack, Progress, Text } from "@chakra-ui/react";
import { Icon } from "@chakra-ui/react";
import { useRouter } from "next/router";
import { useTranslation } from "react-i18next";
import { LuArrowUpRight, LuDownload } from "react-icons/lu";
import AdvancedCard from "@/components/common/advanced-card";
import { useTaskContext } from "@/contexts/task";

const DownloadProgressCard = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const { tasks, generalPercent } = useTaskContext();

  const activeTaskGroups = tasks.filter(
    (group) => group.progress !== undefined && group.progress < 100
  );

  if (activeTaskGroups.length === 0) return null;

  // generalPercent is undefined while no download group is in Started state
  // (paused/failed groups keep counting as active). Never fall back to 0:
  // render an indeterminate bar instead of a frozen 0%.
  // The card is only rendered while some group is unfinished, so 100% is not a
  // truthful value here: cap the rounded value at 99 so 99.6% is never shown
  // as 100% while downloads are still running.
  const percent =
    generalPercent === undefined || !Number.isFinite(generalPercent)
      ? undefined
      : Math.max(0, Math.min(Math.round(generalPercent), 99));

  return (
    <AdvancedCard
      data-launch="download-progress"
      data-progress-state={percent === undefined ? "unknown" : "known"}
      role="button"
      tabIndex={0}
      p={3}
      cursor="pointer"
      onClick={() => router.push("/downloads")}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          router.push("/downloads");
        }
      }}
    >
      <HStack spacing={2} mb={2} minH="1.25rem">
        <Icon as={LuDownload} boxSize={4} flexShrink={0} />
        <Text fontSize="sm" fontWeight="bold" noOfLines={1} flexShrink={0}>
          {t("LaunchPage.downloadProgress.title")}
        </Text>
        <Text
          fontSize="xs"
          className="secondary-text"
          ml="auto"
          minW={0}
          noOfLines={1}
          textAlign="right"
        >
          {t("LaunchPage.downloadProgress.groups", {
            count: activeTaskGroups.length,
          })}
        </Text>
        <Icon as={LuArrowUpRight} boxSize={3.5} flexShrink={0} />
      </HStack>
      <HStack spacing={2} minH="1rem">
        <Progress
          value={percent}
          isIndeterminate={percent === undefined}
          size="xs"
          flex={1}
          borderRadius="full"
        />
        <Text fontSize="xs" fontWeight="bold" minW="2.5rem" textAlign="right">
          {percent === undefined ? "—" : `${percent}%`}
        </Text>
      </HStack>
    </AdvancedCard>
  );
};

export default DownloadProgressCard;
