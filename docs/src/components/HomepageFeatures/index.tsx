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
    title: 'Cross Platform',
    Svg: require('@site/static/img/twemoji/laptop.svg').default,
    description: (
      <>
        Dingus is written in Rust, providing a single, cross-platform binary that works on Windows, macOS, and Linux, and doesn't rely on any specific shell.
      </>
    ),
  },
  {
    title: 'Familiar Interface',
    Svg: require('@site/static/img/twemoji/keyboard.svg').default,
    description: (
      <>
        Dingus presents your tasks and variables in a POSIX-style interface, offering a familiar and consistent user experience.
      </>
    ),
  },
  {
    title: 'Simple Configuration',
    Svg: require('@site/static/img/twemoji/wrench.svg').default,
    description: (
      <>
        Define your commands and variables in a simple, human-readable YAML configuration file.
      </>
    ),
  },
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
