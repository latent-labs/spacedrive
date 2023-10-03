import { MotiView } from 'moti';
import { CaretRight } from 'phosphor-react-native';
import { PropsWithChildren, useReducer } from 'react';
import { Pressable, StyleProp, Text, TextStyle, View, ViewStyle } from 'react-native';
import { tw } from '~/lib/tailwind';

import { AnimatedHeight } from '../animation/layout';

type CollapsibleViewProps = PropsWithChildren<{
	title: string;
	titleStyle?: StyleProp<TextStyle>;
	containerStyle?: StyleProp<ViewStyle>;
}>;

const CollapsibleView = ({ title, titleStyle, containerStyle, children }: CollapsibleViewProps) => {
	const [hide, toggle] = useReducer((hide) => !hide, false);

	return (
		<View style={containerStyle}>
			<Pressable onPress={toggle} style={tw`flex flex-row items-center justify-between pr-3`}>
				<Text style={titleStyle} selectable={false}>
					{title}
				</Text>
				<MotiView
					animate={{ rotateZ: hide ? '0deg' : '90deg' }}
					transition={{ type: 'timing', duration: 150 }}
				>
					<CaretRight color="white" weight="bold" size={16} />
				</MotiView>
			</Pressable>
			<AnimatedHeight hide={hide}>{children}</AnimatedHeight>
		</View>
	);
};

export default CollapsibleView;
