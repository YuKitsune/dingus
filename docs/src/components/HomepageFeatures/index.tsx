import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  Svg: React.ComponentType<React.ComponentProps<'svg'>>;
  description: JSX.Element;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Script Consolidation',
    Svg: require('@site/static/img/twemoji/package.svg').default,
    description: (
      <>
        Keep all your build, CI, and configuration scripts in one place with Dingus, ensuring every developer has access to the same streamlined workflows.
      </>
    ),
  },
  {
    title: 'Consistent User Experience',
    Svg: require('@site/static/img/twemoji/globe.svg').default,
    description: (
      <>
        Dingus provides a POSIX-style interface for your commands, offering a familiar and consistent user experience.
      </>
    ),
  },
  {
    title: 'Streamlined Onboarding',
    Svg: require('@site/static/img/twemoji/rocket.svg').default,
    description: (
      <>
        Bring new contributors up to speed with a single, well-documented YAML file, reducing onboarding time and ensuring consistency in development environments.
      </>
    ),
  },
  // {
  //   title: 'Collaboration and Sharing',
  //   Svg: require('@site/static/img/twemoji/handshake.svg').default,
  //   description: (
  //     <>
  //       Easily share your configurations with team members or the community, and extend others' configurations to fit your needs.
  //     </>
  //   ),
  // },
  // {
  //   title: 'Remote Execution',
  //   Svg: require('@site/static/img/twemoji/satellite.svg').default,
  //   description: (
  //     <>
  //       Execute tasks on remote machines via SSH with Dingus, simplifying remote infrastructure management and routine maintenance tasks.
  //     </>
  //   ),
  // },
  // {
  //   title: 'Dotfiles',
  //   Svg: require('@site/static/img/twemoji/folder.svg').default,
  //   description: (
  //     <>
  //       Replace bash aliases and functions with Dingus to create a cross-platform, shell-agnostic solution with native support for flags, arguments, and more.
  //     </>
  //   ),
  // }
];

function Feature({title, Svg, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center">
        <Svg className={styles.featureSvg} role="img" />
      </div>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): JSX.Element {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
