import { HStack, Progress, Text } from "@chakra-ui/react";
import { useRouter } from "next/router";
import { useTranslation } from "react-i18next";
import { LuArrowUpRight, LuDownload } from "react-icons/lu";
import { Icon } from "@chakra-ui/react";
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

  const percent = Math.min(Math.round(generalPercent ?? 0), 100);

  return (
    <AdvancedCard
      data-launch="download-progress"
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
      <HStack spacing={2} mb={2}>
        <Icon as={LuDownload} boxSize={4} />
        <Text fontSize="sm" fontWeight="bold">
          {t("LaunchPage.downloadProgress.title")}
        </Text>
        <Text fontSize="xs" className="secondary-text" ml="auto">
          {t("LaunchPage.downloadProgress.groups", {
            count: activeTaskGroups.length,
          })}
        </Text>
        <Icon as={LuArrowUpRight} boxSize={3.5} />
      </HStack>
      <HStack spacing={2}>
        <Progress value={percent} size="xs" flex={1} borderRadius="full" />
        <Text fontSize="xs" fontWeight="bold" minW="2.5rem" textAlign="right">
          {percent}%
        </Text>
      </HStack>
    </AdvancedCard>
  );
};

export default DownloadProgressCard;
