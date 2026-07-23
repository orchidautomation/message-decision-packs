import { Composition } from "remotion";
import { ProposalFlowVideo } from "./ProposalFlowVideo";

export const RemotionRoot: React.FC = () => {
  return (
    <Composition
      id="ProposalFlow"
      component={ProposalFlowVideo}
      durationInFrames={1050}
      fps={30}
      width={1920}
      height={1080}
      defaultProps={{}}
    />
  );
};
