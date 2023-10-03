import React from 'react';
import { Controller } from 'react-hook-form';
import { Pressable, Text, View, ViewStyle } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

import { useOnboardingContext } from './context';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

type RadioButtonProps = {
	title: string;
	description: string;
	isSelected: boolean;
	style?: ViewStyle;
};

// Make this a component?
const RadioButton = ({ title, description, isSelected, style }: RadioButtonProps) => {
	return (
		<View
			style={twStyle(
				'flex w-full flex-row items-center rounded-md border border-app-line bg-app-box/50 p-3',
				style
			)}
		>
			<View
				style={twStyle(
					'mr-2.5 h-5 w-5 items-center justify-center rounded-full',
					isSelected ? 'bg-accent' : 'bg-gray-900'
				)}
			>
				{isSelected && <View style={tw`h-1.5 w-1.5 rounded-full bg-white`} />}
			</View>
			<View style={tw`flex-1`}>
				<Text style={tw`text-base font-bold text-ink`}>{title}</Text>
				<Text style={tw`text-sm text-ink-faint`}>{description}</Text>
			</View>
		</View>
	);
};

const PrivacyScreen = () => {
	const { forms, submit } = useOnboardingContext();

	const form = forms.useForm('Privacy');

	return (
		<OnboardingContainer>
			<OnboardingTitle>Your Privacy</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>
				Spacedrive is built for privacy, that's why we're open source and local first. So
				we'll make it very clear what data is shared with us.
			</OnboardingDescription>
			<View style={tw`w-full`}>
				<Controller
					name="shareTelemetry"
					control={form.control}
					render={({ field: { onChange, value } }) => (
						<>
							<Pressable onPress={() => onChange('share-telemetry')}>
								<RadioButton
									title="Share anonymous usage"
									description="Share completely anonymous telemetry data to help the developers improve the app"
									isSelected={value === 'share-telemetry'}
									style={tw`mb-3 mt-4`}
								/>
							</Pressable>
							<Pressable
								testID="share-minimal"
								onPress={() => onChange('minimal-telemetry')}
							>
								<RadioButton
									title="Share the bare minimum"
									description="Only share that I am an active user of Spacedrive and a few technical bits"
									isSelected={value === 'minimal-telemetry'}
								/>
							</Pressable>
						</>
					)}
				/>
			</View>
			<Button variant="accent" size="sm" onPress={form.handleSubmit(submit)} style={tw`mt-6`}>
				<Text style={tw`text-center text-base font-medium text-ink`}>Continue</Text>
			</Button>
		</OnboardingContainer>
	);
};

export default PrivacyScreen;
